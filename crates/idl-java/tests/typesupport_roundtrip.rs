//! Bug J #65 — Java TypeSupport encode→decode round-trip tests.
//!
//! For each conformance fixture that exercises an aggregate member kind
//! (enum, nested struct, typedef, sequence-of-sequence, fixed array, union,
//! map, mixed combo) this test:
//!   1. generates the `--java` files with the real backend,
//!   2. writes a small `Main.java` driver that builds a sample, encodes it via
//!      `<Name>TypeSupport.INSTANCE`, decodes the bytes back, and asserts every
//!      field round-trips (System.exit(1) on mismatch),
//!   3. compiles generated + runtime + driver with `javac`, and
//!   4. runs the driver; a non-zero exit fails the test.
//!
//! This proves an actual wire round-trip recovers the data — not just that the
//! generated code compiles. Skipped (passes vacuously) when `javac` is absent.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_java::{JavaGenOptions, generate_java_files};

fn javac_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect_java(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_java(&path, out);
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
}

/// Compiled runtime classpath (java-omgdds + idl-java/runtime). `None` when no
/// `javac` is available.
fn runtime_classes() -> Option<&'static Path> {
    static CELL: OnceLock<Option<(tempfile::TempDir, PathBuf)>> = OnceLock::new();
    CELL.get_or_init(|| {
        if !javac_available() {
            eprintln!("WARNING: skipping Java round-trip tests, no javac in PATH");
            return None;
        }
        let tmp = tempfile::tempdir().ok()?;
        let out = tmp.path().join("classes");
        std::fs::create_dir_all(&out).ok()?;
        let mut srcs = Vec::new();
        collect_java(
            &manifest().join("../java-omgdds/java/src/main/java"),
            &mut srcs,
        );
        collect_java(&manifest().join("runtime"), &mut srcs);
        let output = Command::new("javac")
            .arg("-nowarn")
            .arg("-d")
            .arg(&out)
            .args(&srcs)
            .output()
            .ok()?;
        assert!(
            output.status.success(),
            "runtime failed to compile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Some((tmp, out))
    })
    .as_ref()
    .map(|(_, p)| p.as_path())
}

/// Generates Java for `idl`, drops in `main_java` (a `conf.Main` driver with a
/// `public static void main`), compiles everything, runs `conf.Main`, and
/// returns Ok(()) if it exits 0.
fn run_roundtrip(idl: &str, main_java: &str) {
    run_roundtrip_pkg(idl, "conf", "Main", main_java);
}

/// Like [`run_roundtrip`] but the driver lives in package `pkg` with class
/// `main_class` — used for fixtures whose module is not `conf`.
fn run_roundtrip_pkg(idl: &str, pkg: &str, main_class: &str, main_java: &str) {
    let Some(classes) = runtime_classes() else {
        return; // no javac — skip
    };

    let ast = zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse");
    let files = generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
    assert!(!files.is_empty(), "no Java files generated for IDL");

    let tmp = tempfile::tempdir().expect("tmp");
    let mut paths = Vec::new();
    for f in &files {
        let dir = if f.package_path.is_empty() {
            tmp.path().to_path_buf()
        } else {
            tmp.path().join(f.package_path.replace('.', "/"))
        };
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join(format!("{}.java", f.class_name));
        std::fs::write(&path, &f.source).expect("write");
        paths.push(path);
    }
    let driver_dir = tmp.path().join(pkg.replace('.', "/"));
    std::fs::create_dir_all(&driver_dir).expect("mkdir driver");
    let main_path = driver_dir.join(format!("{main_class}.java"));
    std::fs::write(&main_path, main_java).expect("write main");
    paths.push(main_path);

    let out_dir = tmp.path().join("out");
    let compile = Command::new("javac")
        .arg("-nowarn")
        .arg("-classpath")
        .arg(classes)
        .arg("-d")
        .arg(&out_dir)
        .args(&paths)
        .output()
        .expect("javac");
    assert!(
        compile.status.success(),
        "javac FAILED:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new("java")
        .arg("-classpath")
        .arg(format!("{}:{}", out_dir.display(), classes.display()))
        .arg(format!("{pkg}.{main_class}"))
        .output()
        .expect("java run");
    assert!(
        run.status.success(),
        "round-trip driver FAILED (exit {:?}):\n--- stdout ---\n{}\n--- stderr ---\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

fn fixture(name: &str) -> String {
    let p = manifest()
        .join("../../tools/idlc/tests/conformance/fixtures")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read fixture {}: {e}", p.display()))
}

const ENUMS_IDL: &str = r#"
module conf {
  enum Color { RED, GREEN, BLUE };
  enum Sparse { @value(1) ONE, @value(2) TWO, @value(8) EIGHT };
  struct UsesEnum { Color primary; Sparse code; };
};
"#;

#[test]
fn roundtrip_enum_in_struct() {
    run_roundtrip(
        ENUMS_IDL,
        r#"package conf;
public class Main {
  public static void main(String[] a) {
    UsesEnum u = new UsesEnum();
    u.setPrimary(Color.BLUE);
    u.setCode(Sparse.EIGHT);
    byte[] b = UsesEnumTypeSupport.INSTANCE.encode(u);
    UsesEnum r = UsesEnumTypeSupport.INSTANCE.decode(b);
    if (r.getPrimary() != Color.BLUE) { System.err.println("primary="+r.getPrimary()); System.exit(1); }
    if (r.getCode() != Sparse.EIGHT) { System.err.println("code="+r.getCode()); System.exit(1); }
  }
}
"#,
    );
}

const NESTED_IDL: &str = r#"
module conf {
  struct Inner { long x; long y; };
  struct Middle { Inner a; Inner b; };
  struct Outer { Middle m; Inner direct; };
};
"#;

#[test]
fn roundtrip_nested_struct() {
    run_roundtrip(
        NESTED_IDL,
        r#"package conf;
public class Main {
  static Inner mk(int x, int y) { Inner i = new Inner(); i.setX(x); i.setY(y); return i; }
  public static void main(String[] a) {
    Outer o = new Outer();
    Middle m = new Middle();
    m.setA(mk(1, 2));
    m.setB(mk(3, 4));
    o.setM(m);
    o.setDirect(mk(5, 6));
    byte[] b = OuterTypeSupport.INSTANCE.encode(o);
    Outer r = OuterTypeSupport.INSTANCE.decode(b);
    if (r.getM().getA().getX() != 1 || r.getM().getA().getY() != 2) { System.err.println("a"); System.exit(1); }
    if (r.getM().getB().getX() != 3 || r.getM().getB().getY() != 4) { System.err.println("b"); System.exit(1); }
    if (r.getDirect().getX() != 5 || r.getDirect().getY() != 6) { System.err.println("direct"); System.exit(1); }
  }
}
"#,
    );
}

const TYPEDEF_IDL: &str = r#"
module conf {
  typedef double  CurrentInAmpsType;
  typedef CurrentInAmpsType  ChargeCurrentType;
  typedef sequence<long>     LongSeq;
  struct UsesTypedefs {
    CurrentInAmpsType battery;
    ChargeCurrentType charger;
    LongSeq           samples;
  };
};
"#;

#[test]
fn roundtrip_typedef_members() {
    run_roundtrip(
        TYPEDEF_IDL,
        r#"package conf;
import java.util.*;
public class Main {
  public static void main(String[] a) {
    UsesTypedefs u = new UsesTypedefs();
    u.setBattery(new CurrentInAmpsType(12.5));
    // alias chain: ChargeCurrentType wraps a CurrentInAmpsType (one-level wrappers).
    u.setCharger(new ChargeCurrentType(new CurrentInAmpsType(3.25)));
    LongSeq s = new LongSeq();
    s.value(Arrays.asList(10, 20, 30));
    u.setSamples(s);
    byte[] b = UsesTypedefsTypeSupport.INSTANCE.encode(u);
    UsesTypedefs r = UsesTypedefsTypeSupport.INSTANCE.decode(b);
    if (r.getBattery().value() != 12.5) { System.err.println("battery="+r.getBattery().value()); System.exit(1); }
    if (r.getCharger().value().value() != 3.25) { System.err.println("charger"); System.exit(1); }
    List<Integer> got = r.getSamples().value();
    if (got.size() != 3 || got.get(0) != 10 || got.get(2) != 30) { System.err.println("samples="+got); System.exit(1); }
  }
}
"#,
    );
}

const SEQ_IDL: &str = r#"
module conf {
  struct Item { long id; };
  struct Sequences {
    sequence<long>            unbounded;
    sequence<Item>            items;
    sequence<sequence<long> > nested;
    sequence<string>          names;
  };
};
"#;

#[test]
fn roundtrip_sequences_incl_nested_and_struct() {
    run_roundtrip(
        SEQ_IDL,
        r#"package conf;
import java.util.*;
public class Main {
  public static void main(String[] a) {
    Sequences s = new Sequences();
    s.setUnbounded(Arrays.asList(1, 2, 3));
    Item it = new Item(); it.setId(99);
    s.setItems(Arrays.asList(it));
    s.setNested(Arrays.asList(Arrays.asList(7, 8), Arrays.asList(9)));
    s.setNames(Arrays.asList("alpha", "beta"));
    byte[] b = SequencesTypeSupport.INSTANCE.encode(s);
    Sequences r = SequencesTypeSupport.INSTANCE.decode(b);
    if (!r.getUnbounded().equals(Arrays.asList(1,2,3))) { System.err.println("unbounded="+r.getUnbounded()); System.exit(1); }
    if (r.getItems().size() != 1 || r.getItems().get(0).getId() != 99) { System.err.println("items"); System.exit(1); }
    List<List<Integer>> n = r.getNested();
    if (n.size() != 2 || !n.get(0).equals(Arrays.asList(7,8)) || !n.get(1).equals(Arrays.asList(9))) { System.err.println("nested="+n); System.exit(1); }
    if (!r.getNames().equals(Arrays.asList("alpha","beta"))) { System.err.println("names="+r.getNames()); System.exit(1); }
  }
}
"#,
    );
}

const ARRAYS_IDL: &str = r#"
module conf {
  struct Point { long x; long y; };
  struct Arrays_ {
    long    vec[3];
    long    grid[2][2];
    Point   shape[2];
  };
};
"#;

#[test]
fn roundtrip_fixed_arrays_incl_multidim_and_struct() {
    run_roundtrip(
        ARRAYS_IDL,
        r#"package conf;
public class Main {
  static Point pt(int x, int y) { Point p = new Point(); p.setX(x); p.setY(y); return p; }
  public static void main(String[] a) {
    Arrays_ s = new Arrays_();
    s.setVec(new int[]{1, 2, 3});
    s.setGrid(new int[][]{{10, 11}, {12, 13}});
    s.setShape(new Point[]{pt(4,5), pt(6,7)});
    byte[] b = Arrays_TypeSupport.INSTANCE.encode(s);
    Arrays_ r = Arrays_TypeSupport.INSTANCE.decode(b);
    int[] v = r.getVec();
    if (v.length != 3 || v[0] != 1 || v[2] != 3) { System.err.println("vec"); System.exit(1); }
    int[][] g = r.getGrid();
    if (g[0][0] != 10 || g[0][1] != 11 || g[1][0] != 12 || g[1][1] != 13) { System.err.println("grid"); System.exit(1); }
    Point[] sh = r.getShape();
    if (sh.length != 2 || sh[0].getX() != 4 || sh[1].getY() != 7) { System.err.println("shape"); System.exit(1); }
  }
}
"#,
    );
}

const MAPS_IDL: &str = r#"
module conf {
  struct Entry { long v; };
  struct Maps {
    map<string, long>    byName;
    map<long, Entry>     byId;
  };
};
"#;

#[test]
fn roundtrip_maps_primitive_and_struct_value() {
    run_roundtrip(
        MAPS_IDL,
        r#"package conf;
import java.util.*;
public class Main {
  public static void main(String[] a) {
    Maps m = new Maps();
    Map<String,Integer> byName = new LinkedHashMap<>();
    byName.put("one", 1); byName.put("two", 2);
    m.setByName(byName);
    Map<Integer,Entry> byId = new LinkedHashMap<>();
    Entry e = new Entry(); e.setV(42); byId.put(7, e);
    m.setById(byId);
    byte[] b = MapsTypeSupport.INSTANCE.encode(m);
    Maps r = MapsTypeSupport.INSTANCE.decode(b);
    if (r.getByName().size() != 2 || r.getByName().get("one") != 1 || r.getByName().get("two") != 2) { System.err.println("byName="+r.getByName()); System.exit(1); }
    if (r.getById().size() != 1 || r.getById().get(7).getV() != 42) { System.err.println("byId="+r.getById()); System.exit(1); }
  }
}
"#,
    );
}

const UNION_IDL: &str = r#"
module conf {
  enum Kind { K_A, K_B, K_C };
  union IntUnion switch (long) {
    case 0: long    asLong;
    case 1:
    case 2: double  asDouble;
    default: string asString;
  };
  union EnumUnion switch (Kind) {
    case K_A: long  a;
    case K_B: short b;
    default:  octet other;
  };
};
"#;

#[test]
fn roundtrip_unions_int_and_enum_discriminator() {
    run_roundtrip(
        UNION_IDL,
        r#"package conf;
public class Main {
  public static void main(String[] a) {
    // integral discriminator: explicit case
    byte[] b1 = IntUnionTypeSupport.INSTANCE.encode(new IntUnion.AsLong(123));
    IntUnion r1 = IntUnionTypeSupport.INSTANCE.decode(b1);
    if (!(r1 instanceof IntUnion.AsLong al) || al.asLong() != 123) { System.err.println("asLong="+r1); System.exit(1); }
    // multi-label branch (case 1/2)
    byte[] b2 = IntUnionTypeSupport.INSTANCE.encode(new IntUnion.AsDouble(2.5));
    IntUnion r2 = IntUnionTypeSupport.INSTANCE.decode(b2);
    if (!(r2 instanceof IntUnion.AsDouble ad) || ad.asDouble() != 2.5) { System.err.println("asDouble="+r2); System.exit(1); }
    // default branch (string)
    byte[] b3 = IntUnionTypeSupport.INSTANCE.encode(new IntUnion.AsString("hi"));
    IntUnion r3 = IntUnionTypeSupport.INSTANCE.decode(b3);
    if (!(r3 instanceof IntUnion.AsString as) || !as.asString().equals("hi")) { System.err.println("asString="+r3); System.exit(1); }
    // enum discriminator
    byte[] b4 = EnumUnionTypeSupport.INSTANCE.encode(new EnumUnion.A(77));
    EnumUnion r4 = EnumUnionTypeSupport.INSTANCE.decode(b4);
    if (!(r4 instanceof EnumUnion.A ea) || ea.a() != 77) { System.err.println("A="+r4); System.exit(1); }
    byte[] b5 = EnumUnionTypeSupport.INSTANCE.encode(new EnumUnion.B((short)9));
    EnumUnion r5 = EnumUnionTypeSupport.INSTANCE.decode(b5);
    if (!(r5 instanceof EnumUnion.B eb) || eb.b() != 9) { System.err.println("B="+r5); System.exit(1); }
  }
}
"#,
    );
}

// ---------------------------------------------------------------------------
// Round-trips driven by the *committed* conformance fixtures (proves the real
// fixture IDL — not just hand-written IDL — survives encode→decode).
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_fixture_06_typedefs_incl_matrix_array_alias() {
    // 06_typedefs.idl: includes `typedef long Matrix3[3][3]` — a typedef whose
    // underlying type is a fixed multi-dim array (wrapper over long[][]).
    run_roundtrip(
        &fixture("06_typedefs.idl"),
        r#"package conf;
import java.util.*;
public class Main {
  public static void main(String[] a) {
    UsesTypedefs u = new UsesTypedefs();
    u.setBattery(new CurrentInAmpsType(7.5));
    u.setCharger(new ChargeCurrentType(new CurrentInAmpsType(8.25)));
    LongSeq ls = new LongSeq(); ls.value(Arrays.asList(1, 2, 3, 4)); u.setSamples(ls);
    int[][] grid = new int[][]{{1,2,3},{4,5,6},{7,8,9}};
    u.setTransform(new Matrix3(grid));
    byte[] b = UsesTypedefsTypeSupport.INSTANCE.encode(u);
    UsesTypedefs r = UsesTypedefsTypeSupport.INSTANCE.decode(b);
    if (r.getBattery().value() != 7.5) { System.err.println("battery"); System.exit(1); }
    if (r.getCharger().value().value() != 8.25) { System.err.println("charger"); System.exit(1); }
    if (!r.getSamples().value().equals(Arrays.asList(1,2,3,4))) { System.err.println("samples="+r.getSamples().value()); System.exit(1); }
    int[][] g = r.getTransform().value();
    if (g[0][0] != 1 || g[1][1] != 5 || g[2][2] != 9 || g[2][0] != 7) { System.err.println("transform"); System.exit(1); }
  }
}
"#,
    );
}

#[test]
fn roundtrip_fixture_20_mixed_combo() {
    // 20_mixed_combo.idl: keyed @appendable Telemetry combining enum + typedef +
    // sequence<struct> + union + map + @optional + fixed array + bounded string.
    run_roundtrip_pkg(
        &fixture("20_mixed_combo.idl"),
        "combo",
        "Main",
        r#"package combo;
import java.util.*;
public class Main {
  public static void main(String[] a) {
    Telemetry t = new Telemetry();
    t.setUnitId(42);
    t.setRegion("eu-west");
    t.setMode(Mode.MODE_ACTIVE);
    t.setBatteryCurrent(new CurrentInAmpsType(13.5));
    Sample s0 = new Sample(); s0.setSeq(1); s0.setValue(1.5);
    Sample s1 = new Sample(); s1.setSeq(2); s1.setValue(2.5);
    t.setHistory(Arrays.asList(s0, s1));
    t.setReading(new Reading.ActiveRate(99.0));
    Map<String,Integer> c = new LinkedHashMap<>(); c.put("ok", 5); c.put("err", 1);
    t.setCounters(c);
    t.setCalibration(Optional.of(0.125));
    t.setWindow(new int[]{10, 20, 30, 40});

    byte[] b = TelemetryTypeSupport.INSTANCE.encode(t);
    Telemetry r = TelemetryTypeSupport.INSTANCE.decode(b);

    if (r.getUnitId() != 42) { System.err.println("unitId="+r.getUnitId()); System.exit(1); }
    if (!r.getRegion().equals("eu-west")) { System.err.println("region="+r.getRegion()); System.exit(1); }
    if (r.getMode() != Mode.MODE_ACTIVE) { System.err.println("mode="+r.getMode()); System.exit(1); }
    if (r.getBatteryCurrent().value() != 13.5) { System.err.println("battery"); System.exit(1); }
    if (r.getHistory().size() != 2 || r.getHistory().get(1).getSeq() != 2 || r.getHistory().get(1).getValue() != 2.5) { System.err.println("history"); System.exit(1); }
    if (!(r.getReading() instanceof Reading.ActiveRate ar) || ar.activeRate() != 99.0) { System.err.println("reading="+r.getReading()); System.exit(1); }
    if (r.getCounters().get("ok") != 5 || r.getCounters().get("err") != 1) { System.err.println("counters="+r.getCounters()); System.exit(1); }
    if (!r.getCalibration().isPresent() || r.getCalibration().get() != 0.125) { System.err.println("calibration="+r.getCalibration()); System.exit(1); }
    int[] w = r.getWindow();
    if (w.length != 4 || w[0] != 10 || w[3] != 40) { System.err.println("window"); System.exit(1); }

    // keyHash must be stable and non-trivial for a keyed type.
    byte[] kh = TelemetryTypeSupport.INSTANCE.keyHash(t);
    if (kh.length != 16) { System.err.println("keyHash len"); System.exit(1); }
  }
}
"#,
    );
}

// ===========================================================================
// Adversarial edge-hardening — the cases that break adapters past hello-world.
// Each test is a REAL encode→decode roundtrip through the generated TypeSupport.
// ===========================================================================

// --- Edge 1: EMPTY collections (count=0 must round-trip, not crash) ---------

const EMPTY_IDL: &str = r#"
module conf {
  struct Empties {
    sequence<long>        ub;
    sequence<long, 4>     bnd;
    string                str;
    wstring               wstr;
    map<string, long>     m;
    sequence<string>      names;
  };
};
"#;

#[test]
fn roundtrip_empty_collections() {
    run_roundtrip(
        EMPTY_IDL,
        r#"package conf;
import java.util.*;
public class Main {
  public static void main(String[] a) {
    Empties e = new Empties();
    e.setUb(new ArrayList<>());
    e.setBnd(new ArrayList<>());
    e.setStr("");
    e.setWstr("");
    e.setM(new LinkedHashMap<>());
    e.setNames(new ArrayList<>());
    byte[] b = EmptiesTypeSupport.INSTANCE.encode(e);
    Empties r = EmptiesTypeSupport.INSTANCE.decode(b);
    if (!r.getUb().isEmpty()) { System.err.println("ub="+r.getUb()); System.exit(1); }
    if (!r.getBnd().isEmpty()) { System.err.println("bnd="+r.getBnd()); System.exit(1); }
    if (!r.getStr().equals("")) { System.err.println("str=["+r.getStr()+"]"); System.exit(1); }
    if (!r.getWstr().equals("")) { System.err.println("wstr=["+r.getWstr()+"]"); System.exit(1); }
    if (!r.getM().isEmpty()) { System.err.println("m="+r.getM()); System.exit(1); }
    if (!r.getNames().isEmpty()) { System.err.println("names="+r.getNames()); System.exit(1); }
  }
}
"#,
    );
}

// --- Edge 2: BOUND enforcement (exactly N ok; over N must throw) ------------

const BOUND_IDL: &str = r#"
module conf {
  struct Bounded {
    sequence<long, 3>     seq3;
    string<5>             s5;
    map<string, long, 2>  m2;
  };
};
"#;

#[test]
fn roundtrip_bounds_exact_ok_and_over_throws() {
    run_roundtrip(
        BOUND_IDL,
        r#"package conf;
import java.util.*;
public class Main {
  public static void main(String[] a) {
    // Exactly at the bound: must round-trip.
    Bounded ok = new Bounded();
    ok.setSeq3(Arrays.asList(1, 2, 3));
    ok.setS5("hello");                 // 5 bytes == bound
    Map<String,Integer> m = new LinkedHashMap<>();
    m.put("a", 1); m.put("b", 2);
    ok.setM2(m);
    byte[] b = BoundedTypeSupport.INSTANCE.encode(ok);
    Bounded r = BoundedTypeSupport.INSTANCE.decode(b);
    if (r.getSeq3().size() != 3 || r.getSeq3().get(2) != 3) { System.err.println("seq3"); System.exit(1); }
    if (!r.getS5().equals("hello")) { System.err.println("s5="+r.getS5()); System.exit(1); }
    if (r.getM2().size() != 2) { System.err.println("m2"); System.exit(1); }

    // Over the sequence bound: must THROW (not silently corrupt).
    Bounded over = new Bounded();
    over.setSeq3(Arrays.asList(1, 2, 3, 4));
    over.setS5("ok"); over.setM2(new LinkedHashMap<>());
    boolean threw = false;
    try { BoundedTypeSupport.INSTANCE.encode(over); }
    catch (RuntimeException ex) { threw = true; }
    if (!threw) { System.err.println("over-bound seq did not throw"); System.exit(1); }

    // Over the string bound: must THROW.
    Bounded overS = new Bounded();
    overS.setSeq3(Arrays.asList(1)); overS.setS5("toolong"); overS.setM2(new LinkedHashMap<>());
    boolean threwS = false;
    try { BoundedTypeSupport.INSTANCE.encode(overS); }
    catch (RuntimeException ex) { threwS = true; }
    if (!threwS) { System.err.println("over-bound string did not throw"); System.exit(1); }

    // Over the map bound: must THROW.
    Bounded overM = new Bounded();
    overM.setSeq3(Arrays.asList(1)); overM.setS5("x");
    Map<String,Integer> big = new LinkedHashMap<>();
    big.put("a",1); big.put("b",2); big.put("c",3);
    overM.setM2(big);
    boolean threwM = false;
    try { BoundedTypeSupport.INSTANCE.encode(overM); }
    catch (RuntimeException ex) { threwM = true; }
    if (!threwM) { System.err.println("over-bound map did not throw"); System.exit(1); }
  }
}
"#,
    );
}

// --- Edge 3: DEEP nesting ---------------------------------------------------

const DEEP_IDL: &str = r#"
module conf {
  struct L3 { long v; };
  struct L2 { L3 inner; long tag; };
  struct L1 { L2 mid; long top; };
  struct WithSeq { sequence<long> data; long id; };
  struct DeepNest {
    L1                          chain;          // struct→struct→struct (3 levels)
    sequence<sequence<L3> >     matrix;         // sequence<sequence<struct>>
    map<string, WithSeq>        bucket;         // map<string, struct-with-a-sequence>
  };
};
"#;

#[test]
fn roundtrip_deep_nesting() {
    run_roundtrip(
        DEEP_IDL,
        r#"package conf;
import java.util.*;
public class Main {
  static L3 l3(int v) { L3 x = new L3(); x.setV(v); return x; }
  public static void main(String[] a) {
    DeepNest d = new DeepNest();
    L1 l1 = new L1(); L2 l2 = new L2();
    l2.setInner(l3(7)); l2.setTag(3); l1.setMid(l2); l1.setTop(9);
    d.setChain(l1);
    d.setMatrix(Arrays.asList(Arrays.asList(l3(1), l3(2)), Arrays.asList(l3(3))));
    WithSeq ws = new WithSeq(); ws.setData(Arrays.asList(10, 20, 30)); ws.setId(5);
    Map<String, WithSeq> bucket = new LinkedHashMap<>(); bucket.put("k", ws);
    d.setBucket(bucket);

    byte[] b = DeepNestTypeSupport.INSTANCE.encode(d);
    DeepNest r = DeepNestTypeSupport.INSTANCE.decode(b);
    if (r.getChain().getMid().getInner().getV() != 7) { System.err.println("chain v"); System.exit(1); }
    if (r.getChain().getMid().getTag() != 3 || r.getChain().getTop() != 9) { System.err.println("chain tags"); System.exit(1); }
    List<List<L3>> mx = r.getMatrix();
    if (mx.size() != 2 || mx.get(0).size() != 2 || mx.get(0).get(1).getV() != 2 || mx.get(1).get(0).getV() != 3) { System.err.println("matrix"); System.exit(1); }
    WithSeq gb = r.getBucket().get("k");
    if (gb == null || gb.getId() != 5 || !gb.getData().equals(Arrays.asList(10,20,30))) { System.err.println("bucket="+r.getBucket()); System.exit(1); }
  }
}
"#,
    );
}

// --- Edge 4: @optional of an AGGREGATE (present AND absent) ------------------

const OPT_AGG_IDL: &str = r#"
module conf {
  struct Inner { long x; long y; };
  struct OptAgg {
    @optional sequence<long>     maybeSeq;
    @optional Inner              maybeInner;
    @optional map<string, long>  maybeMap;
    @optional string             maybeStr;
    long                         required;
  };
};
"#;

#[test]
fn roundtrip_optional_aggregate_present_and_absent() {
    run_roundtrip(
        OPT_AGG_IDL,
        r#"package conf;
import java.util.*;
public class Main {
  public static void main(String[] a) {
    // PRESENT
    OptAgg p = new OptAgg();
    p.setMaybeSeq(Optional.of(Arrays.asList(1, 2, 3)));
    Inner in = new Inner(); in.setX(4); in.setY(5);
    p.setMaybeInner(Optional.of(in));
    Map<String,Integer> mm = new LinkedHashMap<>(); mm.put("k", 9);
    p.setMaybeMap(Optional.of(mm));
    p.setMaybeStr(Optional.of("hi"));
    p.setRequired(42);
    OptAgg rp = OptAggTypeSupport.INSTANCE.decode(OptAggTypeSupport.INSTANCE.encode(p));
    if (!rp.getMaybeSeq().isPresent() || !rp.getMaybeSeq().get().equals(Arrays.asList(1,2,3))) { System.err.println("p.seq="+rp.getMaybeSeq()); System.exit(1); }
    if (!rp.getMaybeInner().isPresent() || rp.getMaybeInner().get().getX() != 4 || rp.getMaybeInner().get().getY() != 5) { System.err.println("p.inner"); System.exit(1); }
    if (!rp.getMaybeMap().isPresent() || rp.getMaybeMap().get().get("k") != 9) { System.err.println("p.map="+rp.getMaybeMap()); System.exit(1); }
    if (!rp.getMaybeStr().isPresent() || !rp.getMaybeStr().get().equals("hi")) { System.err.println("p.str"); System.exit(1); }
    if (rp.getRequired() != 42) { System.err.println("p.required"); System.exit(1); }

    // ABSENT — absent stays absent (empty Optional), NOT a zero-value.
    OptAgg q = new OptAgg();
    q.setMaybeSeq(Optional.empty());
    q.setMaybeInner(Optional.empty());
    q.setMaybeMap(Optional.empty());
    q.setMaybeStr(Optional.empty());
    q.setRequired(7);
    OptAgg rq = OptAggTypeSupport.INSTANCE.decode(OptAggTypeSupport.INSTANCE.encode(q));
    if (rq.getMaybeSeq().isPresent()) { System.err.println("q.seq present"); System.exit(1); }
    if (rq.getMaybeInner().isPresent()) { System.err.println("q.inner present"); System.exit(1); }
    if (rq.getMaybeMap().isPresent()) { System.err.println("q.map present"); System.exit(1); }
    if (rq.getMaybeStr().isPresent()) { System.err.println("q.str present"); System.exit(1); }
    if (rq.getRequired() != 7) { System.err.println("q.required"); System.exit(1); }
  }
}
"#,
    );
}

// --- Edge 5: UNION (every branch, default, as seq-element & map-value) -------

const UNION_EDGE_IDL: &str = r#"
module conf {
  union Var switch (long) {
    case 0: long    asLong;
    case 1: double  asDouble;
    default: string asString;
  };
  struct UnionHolder {
    sequence<Var>        list;     // union as a sequence element
    map<string, Var>     byName;   // union as a map value
  };
};
"#;

#[test]
fn roundtrip_union_as_seq_element_and_map_value() {
    run_roundtrip(
        UNION_EDGE_IDL,
        r#"package conf;
import java.util.*;
public class Main {
  public static void main(String[] a) {
    // every branch + default standalone
    if (!(rt(new Var.AsLong(123)) instanceof Var.AsLong al) || al.asLong() != 123) { System.err.println("asLong"); System.exit(1); }
    if (!(rt(new Var.AsDouble(2.5)) instanceof Var.AsDouble ad) || ad.asDouble() != 2.5) { System.err.println("asDouble"); System.exit(1); }
    if (!(rt(new Var.AsString("dft")) instanceof Var.AsString as) || !as.asString().equals("dft")) { System.err.println("asString(default)"); System.exit(1); }

    // union inside a sequence and a map
    UnionHolder h = new UnionHolder();
    h.setList(Arrays.asList(new Var.AsLong(11), new Var.AsString("xx"), new Var.AsDouble(3.0)));
    Map<String,Var> bn = new LinkedHashMap<>();
    bn.put("one", new Var.AsLong(1));
    bn.put("two", new Var.AsString("hi"));
    h.setByName(bn);
    UnionHolder r = UnionHolderTypeSupport.INSTANCE.decode(UnionHolderTypeSupport.INSTANCE.encode(h));
    List<Var> l = r.getList();
    if (l.size() != 3 || !(l.get(0) instanceof Var.AsLong x0) || x0.asLong() != 11) { System.err.println("list0="+l); System.exit(1); }
    if (!(l.get(1) instanceof Var.AsString x1) || !x1.asString().equals("xx")) { System.err.println("list1"); System.exit(1); }
    if (!(l.get(2) instanceof Var.AsDouble x2) || x2.asDouble() != 3.0) { System.err.println("list2"); System.exit(1); }
    Var v1 = r.getByName().get("one");
    Var v2 = r.getByName().get("two");
    if (!(v1 instanceof Var.AsLong y1) || y1.asLong() != 1) { System.err.println("byName one"); System.exit(1); }
    if (!(v2 instanceof Var.AsString y2) || !y2.asString().equals("hi")) { System.err.println("byName two"); System.exit(1); }
  }
  static Var rt(Var v) { return VarTypeSupport.INSTANCE.decode(VarTypeSupport.INSTANCE.encode(v)); }
}
"#,
    );
}

// --- Edge 6: UNICODE (multi-byte UTF-8 + UTF-16 wstring) --------------------

const UNICODE_IDL: &str = r#"
module conf {
  struct Uni {
    string   s;
    wstring  w;
  };
};
"#;

#[test]
fn roundtrip_unicode_string_and_wstring() {
    run_roundtrip(
        UNICODE_IDL,
        // CJK + emoji (emoji is a surrogate pair in UTF-16). We pass the literals
        // as \u escapes so the .java source is plain ASCII.
        "package conf;\n\
public class Main {\n\
  public static void main(String[] a) {\n\
    String s = \"\\u4F60\\u597D\\uD83D\\uDE80world\";\n\
    String w = \"\\u65E5\\u672C\\u8A9E\\uD83C\\uDF1F\";\n\
    Uni u = new Uni(); u.setS(s); u.setW(w);\n\
    Uni r = UniTypeSupport.INSTANCE.decode(UniTypeSupport.INSTANCE.encode(u));\n\
    if (!r.getS().equals(s)) { System.err.println(\"s mismatch len \"+r.getS().length()); System.exit(1); }\n\
    if (!r.getW().equals(w)) { System.err.println(\"w mismatch len \"+r.getW().length()); System.exit(1); }\n\
    if (r.getS().codePointCount(0, r.getS().length()) != s.codePointCount(0, s.length())) { System.err.println(\"s codepoints\"); System.exit(1); }\n\
  }\n\
}\n",
    );
}

// --- Edge 7: ARRAY (array-of-struct, multi-dim, array-of-bounded-string) ----

const ARRAY_EDGE_IDL: &str = r#"
module conf {
  struct Cell { long r; long c; };
  struct ArrEdge {
    Cell       grid[2][2];     // multi-dim array-of-struct (distinct elems)
    string<8>  names[3];       // array of bounded string
    long        cube[2][2][2]; // 3-dim primitive array
  };
};
"#;

#[test]
fn roundtrip_array_of_struct_multidim_and_bounded_string() {
    run_roundtrip(
        ARRAY_EDGE_IDL,
        r#"package conf;
public class Main {
  static Cell cell(int r, int c) { Cell x = new Cell(); x.setR(r); x.setC(c); return x; }
  public static void main(String[] a) {
    ArrEdge e = new ArrEdge();
    e.setGrid(new Cell[][]{{cell(0,0), cell(0,1)}, {cell(1,0), cell(1,1)}});
    e.setNames(new String[]{"al", "bo", "cy"});
    e.setCube(new int[][][]{{{1,2},{3,4}},{{5,6},{7,8}}});
    ArrEdge r = ArrEdgeTypeSupport.INSTANCE.decode(ArrEdgeTypeSupport.INSTANCE.encode(e));
    Cell[][] g = r.getGrid();
    if (g[0][0].getR() != 0 || g[0][0].getC() != 0) { System.err.println("g00"); System.exit(1); }
    if (g[0][1].getR() != 0 || g[0][1].getC() != 1) { System.err.println("g01"); System.exit(1); }
    if (g[1][0].getR() != 1 || g[1][0].getC() != 0) { System.err.println("g10"); System.exit(1); }
    if (g[1][1].getR() != 1 || g[1][1].getC() != 1) { System.err.println("g11"); System.exit(1); }
    String[] n = r.getNames();
    if (n.length != 3 || !n[0].equals("al") || !n[1].equals("bo") || !n[2].equals("cy")) { System.err.println("names"); System.exit(1); }
    int[][][] c = r.getCube();
    if (c[0][0][0] != 1 || c[0][1][1] != 4 || c[1][0][0] != 5 || c[1][1][1] != 8) { System.err.println("cube"); System.exit(1); }
  }
}
"#,
    );
}

// --- Edge 8: EXTREME primitives (min/max/0/-1) ------------------------------

const EXTREME_IDL: &str = r#"
module conf {
  struct Extreme {
    short              i16;
    long               i32;
    long long          i64;
    unsigned short     u16;
    unsigned long      u32;
    unsigned long long u64;
    octet              o;
    float              f;
    double             d;
  };
};
"#;

#[test]
fn roundtrip_extreme_primitive_values() {
    run_roundtrip(
        EXTREME_IDL,
        r#"package conf;
public class Main {
  public static void main(String[] a) {
    Extreme e = new Extreme();
    e.setI16((short) Short.MIN_VALUE);
    e.setI32(Integer.MAX_VALUE);
    e.setI64(Long.MIN_VALUE);
    e.setU16(0xFFFF);                       // unsigned short max → widened int
    e.setU32(0xFFFFFFFFL);                  // unsigned long max  → widened long
    e.setU64(-1L);                          // unsigned long long max bit-pattern
    e.setO((byte) 0xFF);
    e.setF(-3.5f);
    e.setD(1.7976931348623157e308);         // ~Double.MAX
    Extreme r = ExtremeTypeSupport.INSTANCE.decode(ExtremeTypeSupport.INSTANCE.encode(e));
    if (r.getI16() != Short.MIN_VALUE) { System.err.println("i16="+r.getI16()); System.exit(1); }
    if (r.getI32() != Integer.MAX_VALUE) { System.err.println("i32="+r.getI32()); System.exit(1); }
    if (r.getI64() != Long.MIN_VALUE) { System.err.println("i64="+r.getI64()); System.exit(1); }
    if (r.getU16() != 0xFFFF) { System.err.println("u16="+r.getU16()); System.exit(1); }
    if (r.getU32() != 0xFFFFFFFFL) { System.err.println("u32="+r.getU32()); System.exit(1); }
    if (r.getU64() != -1L) { System.err.println("u64="+r.getU64()); System.exit(1); }
    if ((r.getO() & 0xFF) != 0xFF) { System.err.println("o="+r.getO()); System.exit(1); }
    if (r.getF() != -3.5f) { System.err.println("f="+r.getF()); System.exit(1); }
    if (r.getD() != 1.7976931348623157e308) { System.err.println("d="+r.getD()); System.exit(1); }
  }
}
"#,
    );
}

// --- Edge 9: KEYED (two samples, same key, different payload) ---------------

const KEYED_IDL: &str = r#"
module conf {
  @appendable struct Keyed {
    @key long      id;
    @key string    name;
    long           payload;
  };
};
"#;

#[test]
fn roundtrip_keyed_same_key_different_payload() {
    run_roundtrip(
        KEYED_IDL,
        r#"package conf;
import java.util.Arrays;
public class Main {
  public static void main(String[] a) {
    Keyed s1 = new Keyed(); s1.setId(7); s1.setName("node-A"); s1.setPayload(100);
    Keyed s2 = new Keyed(); s2.setId(7); s2.setName("node-A"); s2.setPayload(999);

    // Payload round-trips per sample.
    Keyed r1 = KeyedTypeSupport.INSTANCE.decode(KeyedTypeSupport.INSTANCE.encode(s1));
    Keyed r2 = KeyedTypeSupport.INSTANCE.decode(KeyedTypeSupport.INSTANCE.encode(s2));
    if (r1.getId() != 7 || !r1.getName().equals("node-A") || r1.getPayload() != 100) { System.err.println("r1"); System.exit(1); }
    if (r2.getId() != 7 || !r2.getName().equals("node-A") || r2.getPayload() != 999) { System.err.println("r2"); System.exit(1); }

    // Same @key → identical keyHash regardless of payload.
    byte[] k1 = KeyedTypeSupport.INSTANCE.keyHash(s1);
    byte[] k2 = KeyedTypeSupport.INSTANCE.keyHash(s2);
    if (k1.length != 16 || !Arrays.equals(k1, k2)) { System.err.println("keyHash differs for same key"); System.exit(1); }

    // Different key → different keyHash.
    Keyed s3 = new Keyed(); s3.setId(8); s3.setName("node-A"); s3.setPayload(100);
    byte[] k3 = KeyedTypeSupport.INSTANCE.keyHash(s3);
    if (Arrays.equals(k1, k3)) { System.err.println("keyHash collided for different key"); System.exit(1); }
  }
}
"#,
    );
}

const TYPEDEF_OF_STRUCT_KEY_IDL: &str = r#"
module conf {
  struct Inner { @key long x; long ignored; };
  typedef Inner InnerAlias;
  struct Outer { @key InnerAlias i; long tail; };
};
"#;

// KeyHash correctness regression: a `@key` member whose type is a TYPEDEF
// alias of a struct (not the struct directly) — previously fell through to
// the generic (non-key) encoder, writing the WHOLE nested struct (including
// `ignored`, XCDR2-framed as a struct payload prefixed by a DHEADER of its
// own byte length, e.g. `[len=8][x=7][ignored=99]`) into the KeyHash, instead
// of just the aliased struct's own `@key` subset (here: `x` alone).
// Byte-exact per XTypes 1.3 §7.6.8.4: BE holder <=16 octets -> zero-padded.
#[test]
fn keyhash_byte_exact_typedef_of_struct_dealiases_to_own_key_subset() {
    run_roundtrip(
        TYPEDEF_OF_STRUCT_KEY_IDL,
        r#"package conf;
import java.util.Arrays;
public class Main {
  public static void main(String[] a) {
    Inner i = new Inner(); i.setX(7); i.setIgnored(99);
    Outer o = new Outer(); o.setI(new InnerAlias(i)); o.setTail(5);
    byte[] h = OuterTypeSupport.INSTANCE.keyHash(o);
    byte[] expected = new byte[]{0,0,0,7, 0,0,0,0,0,0,0,0,0,0,0,0};
    if (!Arrays.equals(h, expected)) {
      System.err.println("keyHash=" + Arrays.toString(h) + " expected=" + Arrays.toString(expected));
      System.exit(1);
    }
    // A `tail`- or `ignored`-only change must NOT move the KeyHash (proves
    // those non-key bytes are excluded, not just coincidentally equal).
    Inner i2 = new Inner(); i2.setX(7); i2.setIgnored(42);
    Outer o2 = new Outer(); o2.setI(new InnerAlias(i2)); o2.setTail(999);
    byte[] h2 = OuterTypeSupport.INSTANCE.keyHash(o2);
    if (!Arrays.equals(h, h2)) {
      System.err.println("keyHash moved on non-key change: " + Arrays.toString(h2));
      System.exit(1);
    }
  }
}
"#,
    );
}
