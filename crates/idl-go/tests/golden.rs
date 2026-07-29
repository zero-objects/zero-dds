// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Go backend: string smoke tests (always) + a byte-identity test that
//! compiles+runs the generated Go and compares to the Rust goldens (gated on
//! `go` being on PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::path::Path;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_go::{GoGenOptions, generate_go_module};

const GOLDEN_IDL: &str = "\
@final struct Golden {
    uint32 id;
    uint16 kind;
    octet flags;
    float value;
    uint64 stamp;
    string label;
    sequence<octet> raw;
};";

fn emit(src: &str, opts: &GoGenOptions) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_go_module(&ast, opts).expect("gen")
}

#[test]
fn final_struct_emits_type_and_marshal() {
    let go = emit(GOLDEN_IDL, &GoGenOptions::default());
    assert!(go.contains("package zdgen"), "{go}");
    assert!(go.contains("type Golden struct {"), "{go}");
    assert!(go.contains("\tId uint32"), "{go}");
    assert!(go.contains("\tKind uint16"), "{go}");
    assert!(go.contains("\tFlags byte"), "{go}");
    assert!(go.contains("\tValue float32"), "{go}");
    assert!(go.contains("\tStamp uint64"), "{go}");
    assert!(go.contains("\tLabel string"), "{go}");
    assert!(go.contains("\tRaw []byte"), "{go}");
    assert!(
        go.contains("func (v Golden) MarshalXCDR(endian Endianness) []byte {"),
        "{go}"
    );
    // @final → compact, no DHEADER framing.
    assert!(!go.contains("body := NewWriter"), "{go}");
    assert!(go.contains("w.PutU32(v.Id)"), "{go}");
    assert!(go.contains("w.PutF32(v.Value)"), "{go}");
    assert!(go.contains("w.PutString(v.Label)"), "{go}");
    assert!(go.contains("w.PutSeqU8(v.Raw)"), "{go}");
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let go = emit(
        "@appendable struct S { uint32 a; };",
        &GoGenOptions::default(),
    );
    assert!(go.contains("body := NewWriter(w.Endian)"), "{go}");
    assert!(go.contains("w.PutU32(uint32(len(body.Buf)))"), "{go}");
    assert!(go.contains("w.PutBytes(body.Buf)"), "{go}");
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_int32_type_and_member_marshals() {
    let go = emit(ENUM_IDL, &GoGenOptions::default());
    assert!(go.contains("type Mode int32"), "{go}");
    assert!(go.contains("ModeMODE_IDLE Mode = 0"), "{go}");
    assert!(go.contains("ModeMODE_ACTIVE Mode = 1"), "{go}");
    assert!(go.contains("ModeMODE_FAULT Mode = 2"), "{go}");
    assert!(go.contains("\tKind Mode"), "{go}");
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1):
    // the byte-identity-verified PutU32 path.
    assert!(go.contains("w.PutU32(uint32(v.Kind))"), "{go}");
}

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs the Go toolchain. S{ kind: MODE_FAULT(=2), tail: 0xDEADBEEF }
    // → i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP enum byte test: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(ENUM_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	s := S{Kind: ModeMODE_FAULT, Tail: 0xDEADBEEF}
	fmt.Println(hex.EncodeToString(s.MarshalXCDR(Little)))
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
        1,
    );

    let dir = std::env::temp_dir().join(format!("idlgo_enum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");

    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let hex_out = String::from_utf8_lossy(&out.stdout);
    assert_eq!(hex_out.trim(), "02000000efbeadde", "src:\n{src}");
    let _ = std::fs::remove_dir_all(&dir);
}

const NESTED_IDL: &str = "\
@appendable struct Inner { unsigned short a; unsigned long b; };
@appendable struct Outer { unsigned long id; Inner one; sequence<Inner> many; string label; };";

#[test]
fn nested_struct_emits_marshal_into() {
    let go = emit(NESTED_IDL, &GoGenOptions::default());
    assert!(go.contains("func (v Inner) marshalInto(w *Writer)"), "{go}");
    assert!(go.contains("One Inner"), "{go}");
    assert!(go.contains("Many []Inner"), "{go}");
    // Nested struct member -> marshal into the same writer.
    assert!(go.contains("v.One.marshalInto(body)"), "{go}");
    // sequence<struct> -> collection DHEADER + count + per-element marshalInto.
    assert!(go.contains("e.marshalInto(sub)"), "{go}");
}

#[test]
fn nested_is_byte_identical_vs_rust_golden() {
    // Gated: needs the Go toolchain + GOLDEN_DIR/golden_nested_{le,be}.bin.
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP nested byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP nested byte: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(NESTED_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	o := Outer{
		Id:    0xCAFEBABE,
		One:   Inner{A: 0x1111, B: 0x22223333},
		Many:  []Inner{{A: 0xAAAA, B: 0xBBBBCCCC}, {A: 0xDDDD, B: 0xEEEEFFFF}},
		Label: "nested",
	}
	fmt.Println(hex.EncodeToString(o.MarshalXCDR(Little)))
	fmt.Println(hex.EncodeToString(o.MarshalXCDR(Big)))
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_nested_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    let got_le = lines.next().expect("le").trim();
    let got_be = lines.next().expect("be").trim();
    assert_eq!(
        got_le,
        hex_file(Path::new(&golden_dir).join("golden_nested_le.bin")),
        "LE"
    );
    assert_eq!(
        got_be,
        hex_file(Path::new(&golden_dir).join("golden_nested_be.bin")),
        "BE"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const TYPEDEF_IDL: &str = "\
typedef unsigned long Id;
typedef Id AliasId;
typedef string Label;
typedef sequence<octet> Blob;
@final struct Rec { AliasId id; Label name; Blob data; };";

#[test]
fn typedef_resolves_to_underlying_type() {
    let go = emit(TYPEDEF_IDL, &GoGenOptions::default());
    // typedef chain -> uint32; typedef string -> string; typedef seq<octet> -> []byte.
    assert!(go.contains("Id uint32"), "{go}");
    assert!(go.contains("Name string"), "{go}");
    assert!(go.contains("Data []byte"), "{go}");
}

#[test]
fn typedef_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP typedef byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP typedef byte: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(TYPEDEF_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	r := Rec{Id: 0xCAFEBABE, Name: "typedef", Data: []byte{1, 2, 3}}
	fmt.Println(hex.EncodeToString(r.MarshalXCDR(Little)))
	fmt.Println(hex.EncodeToString(r.MarshalXCDR(Big)))
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_typedef_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_file(Path::new(&golden_dir).join("golden_typedef_le.bin")),
        "LE"
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_file(Path::new(&golden_dir).join("golden_typedef_be.bin")),
        "BE"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const ARRAY_IDL: &str = "\
@final struct Arr { long xs[3]; short m[2][2]; octet bs[4]; };";

#[test]
fn array_emits_fixed_arrays_and_loops() {
    let go = emit(ARRAY_IDL, &GoGenOptions::default());
    assert!(go.contains("Xs [3]int32"), "{go}");
    assert!(go.contains("M [2][2]int16"), "{go}");
    assert!(go.contains("Bs [4]byte"), "{go}");
    assert!(go.contains("for i0 := 0; i0 < 3; i0++"), "{go}");
    assert!(go.contains("for i1 := 0; i1 < 2; i1++"), "{go}");
}

#[test]
fn array_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP array byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP array byte: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(ARRAY_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	a := Arr{
		Xs: [3]int32{0x11111111, 0x22222222, 0x33333333},
		M:  [2][2]int16{{0x0102, 0x0304}, {0x0506, 0x0708}},
		Bs: [4]byte{0xAA, 0xBB, 0xCC, 0xDD},
	}
	fmt.Println(hex.EncodeToString(a.MarshalXCDR(Little)))
	fmt.Println(hex.EncodeToString(a.MarshalXCDR(Big)))
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_array_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_file(Path::new(&golden_dir).join("golden_array_le.bin")),
        "LE"
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_file(Path::new(&golden_dir).join("golden_array_be.bin")),
        "BE"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const UNION_IDL: &str = "\
@final union U switch (long) { case 1: unsigned long a; case 2: unsigned short b; default: octet c; };";

#[test]
fn union_emits_discriminated_struct_and_switch() {
    let go = emit(UNION_IDL, &GoGenOptions::default());
    assert!(go.contains("Disc int32"), "{go}");
    assert!(go.contains("A uint32"), "{go}");
    assert!(go.contains("B uint16"), "{go}");
    assert!(go.contains("C byte"), "{go}");
    assert!(go.contains("switch v.Disc {"), "{go}");
    assert!(go.contains("case 1:"), "{go}");
    assert!(go.contains("case 2:"), "{go}");
    assert!(go.contains("default:"), "{go}");
}

#[test]
fn union_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP union byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP union byte: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(UNION_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	u := U{Disc: 2, B: 0x1234}
	fmt.Println(hex.EncodeToString(u.MarshalXCDR(Little)))
	fmt.Println(hex.EncodeToString(u.MarshalXCDR(Big)))
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_file(Path::new(&golden_dir).join("golden_union_le.bin")),
        "LE"
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_file(Path::new(&golden_dir).join("golden_union_be.bin")),
        "BE"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const MAP_IDL: &str = "\
@final struct HasMap { map<long, unsigned long> m; };";

#[test]
fn map_emits_sorted_key_marshal() {
    let go = emit(MAP_IDL, &GoGenOptions::default());
    assert!(go.contains("M map[int32]uint32"), "{go}");
    assert!(go.contains("sort.Slice(zdKeys"), "{go}");
    assert!(go.contains("\t\"sort\""), "{go}");
}

#[test]
fn map_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP map byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP map byte: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(MAP_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	h := HasMap{M: map[int32]uint32{1: 0x11111111, 2: 0x22222222}}
	fmt.Println(hex.EncodeToString(h.MarshalXCDR(Little)))
	fmt.Println(hex.EncodeToString(h.MarshalXCDR(Big)))
}
"#,
    );
    // The module already imports "math"+"sort" (map); add hex+fmt for the driver.
    let src = src.replacen(
        "\t\"sort\"\n)",
        "\t\"sort\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_map_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_file(Path::new(&golden_dir).join("golden_map_le.bin")),
        "LE"
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_file(Path::new(&golden_dir).join("golden_map_be.bin")),
        "BE"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const MUTABLE_IDL: &str = "\
@mutable struct M { @id(10) unsigned long x; @id(20) string s; @id(30) unsigned short k; };";

#[test]
fn mutable_emits_emheader_framing() {
    let go = emit(MUTABLE_IDL, &GoGenOptions::default());
    assert!(go.contains("body.PutU32(0x4000000a)"), "{go}");
    assert!(go.contains("body.PutU32(0x40000014)"), "{go}");
    assert!(go.contains("body.PutU32(0x4000001e)"), "{go}");
}

#[test]
fn mutable_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP mutable byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP mutable byte: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(MUTABLE_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	m := M{X: 0xDEADBEEF, S: "mut", K: 0x0777}
	fmt.Println(hex.EncodeToString(m.MarshalXCDR(Little)))
	fmt.Println(hex.EncodeToString(m.MarshalXCDR(Big)))
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_mutable_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_file(Path::new(&golden_dir).join("golden_mutable_le.bin")),
        "LE"
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_file(Path::new(&golden_dir).join("golden_mutable_be.bin")),
        "BE"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const WIDE_IDL: &str = "\
@final struct W { wchar c; wstring s; };";
const LD_IDL: &str = "\
@final struct L { long double d; };";

#[test]
fn wide_and_longdouble_emit_types() {
    let w = emit(WIDE_IDL, &GoGenOptions::default());
    assert!(w.contains("C rune"), "{w}");
    assert!(w.contains("S string"), "{w}");
    assert!(w.contains("$w.PutWString") || w.contains("v.S)"), "{w}");
    let l = emit(LD_IDL, &GoGenOptions::default());
    assert!(l.contains("D float64"), "{l}");
    assert!(l.contains("PutLongDouble(v.D)"), "{l}");
}

fn run_go_golden(idl: &str, main_body: &str, golden_stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP {golden_stem} byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP {golden_stem} byte: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(main_body);
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_{golden_stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_file(Path::new(&golden_dir).join(format!("golden_{golden_stem}_le.bin"))),
        "LE"
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_file(Path::new(&golden_dir).join(format!("golden_{golden_stem}_be.bin"))),
        "BE"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn wide_is_byte_identical_vs_rust_golden() {
    run_go_golden(
        WIDE_IDL,
        "\nfunc main() {\n\tw := W{C: 0x03A9, S: \"w\\u03c0\"}\n\tfmt.Println(hex.EncodeToString(w.MarshalXCDR(Little)))\n\tfmt.Println(hex.EncodeToString(w.MarshalXCDR(Big)))\n}\n",
        "wide",
    );
}

#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    run_go_golden(
        LD_IDL,
        "\nfunc main() {\n\tl := L{D: 1.1}\n\tfmt.Println(hex.EncodeToString(l.MarshalXCDR(Little)))\n\tfmt.Println(hex.EncodeToString(l.MarshalXCDR(Big)))\n}\n",
        "longdouble",
    );
}

const KEYHASH_IDL: &str = "\
@final struct K { @key long a; @key unsigned short b; long c; };";

#[test]
fn keyhash_emits_method() {
    let go = emit(KEYHASH_IDL, &GoGenOptions::default());
    assert!(go.contains("func (v K) KeyHash() [16]byte"), "{go}");
    assert!(go.contains("kw := NewWriter(Big)"), "{go}");
}

const NESTED_KEY_IDL: &str = "\
@final struct Inner { @key long x; long ignored; @key long y; };\n\
@final struct Outer { @key Inner i; };";

/// Bug A regression: a `@key` member whose type is itself a struct must
/// expand to ONLY that struct's own `@key` members (x, y), not its full
/// member set — `ignored` must not appear in the KeyHash body.
#[test]
fn nested_struct_key_excludes_non_key_fields() {
    let go = emit(NESTED_KEY_IDL, &GoGenOptions::default());
    let start = go.find("func (v Outer) KeyHash() [16]byte {").expect("KeyHash method");
    let end = go[start..].find("\n}\n").map(|i| start + i).unwrap_or(go.len());
    let body = &go[start..end];
    assert!(body.contains("v.I.X"), "{body}");
    assert!(body.contains("v.I.Y"), "{body}");
    assert!(!body.contains("v.I.Ignored"), "{body}");
    // The full-marshal call must not be reused for the key encoding.
    assert!(!body.contains("v.I.marshalInto"), "{body}");
}

const NESTED_KEY_SMALL_IDL: &str = "\
@final struct Inner { @key octet a; };\n\
@final struct Outer { @key Inner i; };";

/// Bug B regression: `uses_md5` must be given a real `structs` map so a
/// small nested-struct `@key` (here: 1 octet = 1 byte <= 16) resolves and
/// takes the zero-pad branch, not the MD5 branch forced by an unresolvable
/// (empty-map) struct lookup.
#[test]
fn nested_struct_key_small_takes_zero_pad_branch() {
    let go = emit(NESTED_KEY_SMALL_IDL, &GoGenOptions::default());
    let start = go.find("func (v Outer) KeyHash() [16]byte {").expect("KeyHash method");
    let end = go[start..].find("\n}\n").map(|i| start + i).unwrap_or(go.len());
    let body = &go[start..end];
    assert!(body.contains("copy(out[:], kw.Buf)"), "{body}");
    assert!(!body.contains("md5.Sum("), "{body}");
}

#[test]
fn keyhash_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP keyhash: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP keyhash: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(KEYHASH_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	k := K{A: 0x01020304, B: 0x0506}
	h := k.KeyHash()
	fmt.Println(hex.EncodeToString(h[:]))
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_keyhash_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_file(Path::new(&golden_dir).join("golden_keyhash.bin")),
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const KEYHASH_MD5_IDL: &str = "\
@final struct KL { @key long a; @key long b; @key long c; @key long d; @key long e; };";

#[test]
fn keyhash_md5_is_byte_identical_vs_rust_golden() {
    // 5×@key long = 20 PLAIN_CDR2-BE bytes > 16 → MD5 branch (XTypes §7.6.8.4).
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP keyhash_md5: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP keyhash_md5: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(KEYHASH_MD5_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	k := KL{A: 0x01020304, B: 0x05060708, C: 0x090A0B0C, D: 0x0D0E0F10, E: 0x11121314}
	h := k.KeyHash()
	fmt.Println(hex.EncodeToString(h[:]))
}
"#,
    );
    // The module already imports crypto/md5 (block form); inject hex + fmt.
    let src = src.replacen(
        "\t\"math\"\n",
        "\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_keyhash_md5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_file(Path::new(&golden_dir).join("golden_keyhash_md5.bin")),
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Roundtrip decode: UnmarshalXCDR(golden) then MarshalXCDR must reproduce the
/// golden bytes, for both LE and BE. `ty` is the top-level type; `stem` selects
/// golden_<stem>_{le,be}.bin (empty stem = golden_{le,be}.bin).
fn run_roundtrip(idl: &str, ty: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP roundtrip: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP roundtrip: `go` not on PATH");
        return;
    }
    let file = |e: &str| {
        if stem.is_empty() {
            format!("golden_{e}.bin")
        } else {
            format!("golden_{stem}_{e}.bin")
        }
    };
    let lit = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|b| format!("0x{b:02x}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let le = std::fs::read(Path::new(&golden_dir).join(file("le"))).expect("read le");
    let be = std::fs::read(Path::new(&golden_dir).join(file("be"))).expect("read be");
    let mut src = generate_go_module(
        &zerodds_idl::parse(idl, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(&format!(
        "\nfunc main() {{\n\tle := []byte{{{}}}\n\tbe := []byte{{{}}}\n\tfmt.Println(hex.EncodeToString(UnmarshalXCDR{ty}(le, Little).MarshalXCDR(Little)))\n\tfmt.Println(hex.EncodeToString(UnmarshalXCDR{ty}(be, Big).MarshalXCDR(Big)))\n}}\n",
        lit(&le),
        lit(&be)
    ));
    // A map member already pulls in `sort` via a grouped import; otherwise the
    // module has a single `import "math"`. Add hex+fmt for the driver either way.
    let src = if src.contains("\t\"sort\"\n)") {
        src.replacen(
            "\t\"sort\"\n)",
            "\t\"sort\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)",
            1,
        )
    } else {
        src.replacen(
            "import \"math\"\n",
            "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
            1,
        )
    };
    let dir = std::env::temp_dir().join(format!("idlgo_rt_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut lines = stdout.lines();
    let hx = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    assert_eq!(lines.next().expect("le").trim(), hx(&le), "LE roundtrip");
    assert_eq!(lines.next().expect("be").trim(), hx(&be), "BE roundtrip");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn decode_roundtrip_final() {
    run_roundtrip(GOLDEN_IDL, "Golden", "");
}
#[test]
fn decode_roundtrip_nested() {
    run_roundtrip(NESTED_IDL, "Outer", "nested");
}
#[test]
fn decode_roundtrip_array() {
    run_roundtrip(ARRAY_IDL, "Arr", "array");
}
#[test]
fn decode_roundtrip_union() {
    run_roundtrip(UNION_IDL, "U", "union");
}
#[test]
fn decode_roundtrip_map() {
    run_roundtrip(MAP_IDL, "HasMap", "map");
}
#[test]
fn decode_roundtrip_mutable() {
    run_roundtrip(MUTABLE_IDL, "M", "mutable");
}
#[test]
fn decode_roundtrip_wide() {
    run_roundtrip(WIDE_IDL, "W", "wide");
}
#[test]
fn decode_roundtrip_longdouble() {
    run_roundtrip(LD_IDL, "L", "longdouble");
}

fn hex_file(p: std::path::PathBuf) -> String {
    std::fs::read(&p)
        .expect("read golden")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn byte_identity_vs_rust_goldens() {
    // Gated: needs the Go toolchain and the pre-generated goldens.
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP byte_identity: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP byte_identity: `go` not on PATH");
        return;
    }

    // Generate the Go type as `package main`, then append a main() that
    // constructs the @final Golden value and prints LE + BE wire as hex.
    let mut src = generate_go_module(
        &zerodds_idl::parse(GOLDEN_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
import "encoding/hex"
import "fmt"

func main() {
	g := Golden{
		Id:    0xA1B2C3D4,
		Kind:  0x1234,
		Flags: 0x5A,
		Value: 3.5,
		Stamp: 0x0102030405060708,
		Label: "bay-12",
		Raw:   []byte{0xDE, 0xAD, 0xBE, 0xEF},
	}
	fmt.Println(hex.EncodeToString(g.MarshalXCDR(Little)))
	fmt.Println(hex.EncodeToString(g.MarshalXCDR(Big)))
}
"#,
    );
    // Go requires a single import block; merge the appended imports into the
    // prelude's `import "math"` by rewriting to a grouped import.
    let src = src
        .replacen(
            "import \"math\"\n",
            "import (\n\t\"math\"\n\t\"encoding/hex\"\n\t\"fmt\"\n)\n",
            1,
        )
        .replace("\nimport \"encoding/hex\"\nimport \"fmt\"\n", "\n");

    let dir = std::env::temp_dir().join(format!("idlgo_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");

    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    let got_le = lines.next().expect("le line").trim().to_string();
    let got_be = lines.next().expect("be line").trim().to_string();

    let want_le = hex_of(Path::new(&golden_dir).join("golden_le.bin"));
    let want_be = hex_of(Path::new(&golden_dir).join("golden_be.bin"));
    assert_eq!(
        got_le, want_le,
        "LE wire not byte-identical to golden_le.bin"
    );
    assert_eq!(
        got_be, want_be,
        "BE wire not byte-identical to golden_be.bin"
    );
}

fn hex_of(p: std::path::PathBuf) -> String {
    let bytes = std::fs::read(&p).unwrap_or_else(|_| panic!("read {}", p.display()));
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let go = emit(
        "module Telemetry { @final struct Reading { long value; }; };",
        &GoGenOptions::default(),
    );
    assert!(go.contains("type Reading struct {"), "{go}");
    assert!(go.contains("\tValue int32"), "{go}");
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let go = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
        &GoGenOptions::default(),
    );
    assert!(go.contains("type A struct {"), "{go}");
    assert!(go.contains("type B struct {"), "{go}");
}

const KEYWORD_IDL: &str = "\
@final struct chan {
    long a;
};
@appendable struct Holder {
    chan one;
    sequence<chan> many;
};
union select switch(long) {
case 1: chan first;
default: long other;
};";

#[test]
fn keyword_colliding_type_names_are_underscore_escaped() {
    let go = emit(KEYWORD_IDL, &GoGenOptions::default());
    assert!(go.contains("type chan_ struct {"), "{go}");
    assert!(go.contains("func (v chan_) marshalInto(w *Writer)"), "{go}");
    assert!(
        go.contains("func unmarshalFromchan_(r *Reader) chan_"),
        "{go}"
    );
    assert!(go.contains("One chan_"), "{go}");
    assert!(go.contains("Many []chan_"), "{go}");
    assert!(go.contains("v.One.marshalInto(body)"), "{go}");
    assert!(go.contains("v.One = unmarshalFromchan_(r)"), "{go}");
    assert!(go.contains("type select_ struct {"), "{go}");
    assert!(go.contains("First chan_"), "{go}");
    // No bare (unescaped) keyword ever appears as a Go type declaration.
    assert!(!go.contains("type chan struct {"), "{go}");
    assert!(!go.contains("type select struct {"), "{go}");
}

#[test]
fn keyword_colliding_type_names_compile_with_go() {
    if Command::new("go").arg("version").output().is_err() {
        eprintln!("SKIP keyword_colliding_type_names_compile_with_go: `go` not on PATH");
        return;
    }
    let mut src = generate_go_module(
        &zerodds_idl::parse(KEYWORD_IDL, &ParserConfig::default()).expect("parse"),
        &GoGenOptions {
            package_name: "main".to_string(),
        },
    )
    .expect("gen");
    src.push_str(
        r#"
func main() {
	c := chan_{A: 1}
	h := Holder{One: c, Many: []chan_{c}}
	b := h.MarshalXCDR(Little)
	h2 := unmarshalFromHolder(NewReader(b, Little))
	if h2.One.A != 1 {
		panic("roundtrip mismatch")
	}
	s := select_{Disc: 1, First: c}
	_ = s.MarshalXCDR(Little)
	fmt.Println("ok")
}
"#,
    );
    let src = src.replacen(
        "import \"math\"\n",
        "import (\n\t\"math\"\n\t\"fmt\"\n)\n",
        1,
    );
    let dir = std::env::temp_dir().join(format!("idlgo_kw_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let gofile = dir.join("main.go");
    std::fs::write(&gofile, &src).expect("write go");
    let out = Command::new("go")
        .arg("run")
        .arg(&gofile)
        .output()
        .expect("go run");
    assert!(
        out.status.success(),
        "go run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
    let _ = std::fs::remove_dir_all(&dir);
}
