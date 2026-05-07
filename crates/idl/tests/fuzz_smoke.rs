//! Stable-Rust Fuzz-Smoke-Tests fuer den IDL-Parser.
//!
//! Pseudo-random UTF-8-Strings in `zerodds_idl::parse`. Der Parser darf
//! auf keinem Input panicen — nur `Ok(..)` oder `Err(..)` sind
//! erlaubt. Ergaenzt die in `crates/idl/tests/`-Verzeichnis bereits
//! vorhandenen IDL-spezifischen Compliance-Tests um adversarial
//! Input-Robustheit.
//!
//! Spec-Anker: OMG IDL 4.2 — Lexer + Parser-State-Machine.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_idl::config::ParserConfig;

#[derive(Debug, Clone)]
struct XorShift32(u32);

impl XorShift32 {
    fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}

/// Erzeugt einen pseudo-random ASCII-String der Laenge `len`.
/// Beschraenkt auf den printable ASCII-Range plus `\n\t` — der
/// Parser muss UTF-8-validen Input akzeptieren, aber wir wollen
/// hier den Lexer/Parser-Pfad hammern, nicht die UTF-8-Validierung.
fn random_ascii(rng: &mut XorShift32, len: usize) -> String {
    let mut out = String::with_capacity(len);
    while out.len() < len {
        let w = rng.next_u32();
        for shift in 0..4 {
            let b = ((w >> (shift * 8)) & 0x7F) as u8;
            // Map ungueltige Steuerzeichen (ausser \n \t) auf Space.
            let c = match b {
                b'\n' | b'\t' => b as char,
                0..=0x1F | 0x7F => ' ',
                _ => b as char,
            };
            out.push(c);
            if out.len() >= len {
                break;
            }
        }
    }
    out
}

/// Erzeugt pseudo-random Bytes und versucht sie als UTF-8 zu
/// interpretieren. Bei Bad-UTF-8 nehmen wir den lossy-Pfad — auch
/// daraus muss der Parser sauber `Err` liefern.
fn random_utf8_lossy(rng: &mut XorShift32, len: usize) -> String {
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        let w = rng.next_u32().to_le_bytes();
        bytes.extend_from_slice(&w);
    }
    bytes.truncate(len);
    String::from_utf8_lossy(&bytes).into_owned()
}

fn fuzz_parser<F: FnMut(&str)>(seed: u32, iterations: usize, mut f: F) {
    let mut rng = XorShift32::new(seed);
    for i in 0..iterations {
        let len = match i % 8 {
            0 => 0,
            1 => 1,
            2 => 16,
            3 => 64,
            4 => 256,
            5 => 1024,
            6 => 4096,
            _ => 16384,
        };
        let src = if i % 2 == 0 {
            random_ascii(&mut rng, len)
        } else {
            random_utf8_lossy(&mut rng, len)
        };
        f(&src);
    }
}

#[test]
fn fuzz_idl_parse_no_panic() {
    let cfg = ParserConfig::default();
    fuzz_parser(0x4944_4C50, 2_000, |src| {
        let _ = zerodds_idl::parse(src, &cfg);
    });
}

#[test]
fn empty_input_parses_to_empty_spec() {
    let res = zerodds_idl::parse("", &ParserConfig::default());
    // Leere Spec ist ein valider IDL-Korpus ("nichts deklariert").
    assert!(res.is_ok());
}

#[test]
fn single_char_inputs_no_panic() {
    let cfg = ParserConfig::default();
    for c in 0u8..=127 {
        let src = (c as char).to_string();
        let _ = zerodds_idl::parse(&src, &cfg);
    }
}

/// Tief verschachtelte Module: depth `module M { ... }` ineinander.
/// Spec-konform, aber Stack-Overflow-DoS-Vektor; recursion-cap
/// muss greifen oder Parser muss iterativ arbeiten.
///
/// Aktuell mit depth=64 — empirisch unterhalb der Stack-Grenze des
/// rekursiv-deszendierenden Parsers. **Hoehere Tiefen triggern
/// Stack-Overflow** (TS-1-Finding 2026-05-01); siehe
/// `deeply_nested_modules_known_overflow` fuer das ignorierte
/// Reproduktions-Test, und Open-Issue zu Parser-Recursion-Cap.
#[test]
fn deeply_nested_modules_within_safe_depth() {
    let mut src = String::new();
    let depth = 64;
    for i in 0..depth {
        src.push_str(&format!("module M{i} {{ "));
    }
    for _ in 0..depth {
        src.push_str("};");
    }
    let _ = zerodds_idl::parse(&src, &ParserConfig::default());
}

/// Spec-Kontrolltest: 256 nested Module triggern den Pre-Tokenization-
/// Cap (`parser::MAX_NESTING_DEPTH = 64`) — Parser MUSS einen
/// `DepthLimit`-Fehler liefern, nicht in Stack-Overflow laufen.
///
/// TS-1-Finding 1 (gefixt 2026-05-01).
#[test]
fn deeply_nested_modules_rejected_by_depth_cap() {
    let mut src = String::new();
    let depth = 256;
    for i in 0..depth {
        src.push_str(&format!("module M{i} {{ "));
    }
    for _ in 0..depth {
        src.push_str("};");
    }
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::DepthLimit { .. })),
        "expected DepthLimit error, got {res:?}"
    );
}

/// Sehr lange Identifiers (10k Zeichen) — Lexer muss begrenzen
/// oder ohne Allokations-Explosion durchgehen.
#[test]
fn very_long_identifier_no_panic() {
    let ident: String = "a".repeat(10_000);
    let src = format!("struct {ident} {{ long x; }};");
    let _ = zerodds_idl::parse(&src, &ParserConfig::default());
}

/// Realistische Anzahl Annotations: `@final @final ... struct S{};`
/// Mit 50 Annotations laeuft der Parser in <1s durch.
#[test]
fn many_annotations_no_panic_realistic() {
    let mut src = String::new();
    for _ in 0..50 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let _ = zerodds_idl::parse(&src, &ParserConfig::default());
}

/// Spec-Kontrolltest: 100 aufeinanderfolgende Annotations triggern
/// den Pre-Tokenization-Cap (`parser::MAX_CONSECUTIVE_ANNOTATIONS = 64`)
/// — Parser MUSS einen `AnnotationLimit`-Fehler liefern, statt in
/// O(n²) CST-Build-Kosten zu laufen.
///
/// TS-1-Finding 2 (gefixt 2026-05-01).
///
/// Anzahl bewusst niedrig (100, nicht 1000): bei mutation-test-runs
/// die den Cap deaktivieren laeuft Recognize ueber alle Annotations.
/// 1000 wuerde dort den 90s-Mutation-Timeout reissen, 100 nicht.
#[test]
fn many_annotations_rejected_by_annotation_cap() {
    let mut src = String::new();
    for _ in 0..100 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "expected AnnotationLimit, got {res:?}"
    );
}

/// Mutation-Test: depth=MAX_NESTING_DEPTH+1 muss exakt fuer `> MAX`
/// triggern (faengt `>` -> `==` und `>` -> `>=` Mutationen).
/// MAX_NESTING_DEPTH=64; 65 nested {} muss DepthLimit liefern.
#[test]
fn nesting_depth_just_over_cap_rejected() {
    let depth = 65; // = MAX_NESTING_DEPTH + 1
    let mut src = String::new();
    for i in 0..depth {
        src.push_str(&format!("module M{i} {{ "));
    }
    for _ in 0..depth {
        src.push_str("};");
    }
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::DepthLimit { .. })),
        "depth=65 must trigger DepthLimit, got {res:?}"
    );
}

/// Mutation-Test: depth=MAX_NESTING_DEPTH (64) muss NICHT triggern
/// (faengt `>` -> `>=` Mutation, die schon bei 64 erroren wuerde).
#[test]
fn nesting_depth_at_cap_accepted() {
    let depth = 64; // = MAX_NESTING_DEPTH
    let mut src = String::new();
    for i in 0..depth {
        src.push_str(&format!("module M{i} {{ "));
    }
    for _ in 0..depth {
        src.push_str("};");
    }
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    // Erwartet: KEIN DepthLimit-Error. Andere Errors (Lex/Parse) sind
    // hier akzeptabel, der Cap selbst darf nicht greifen.
    assert!(
        !matches!(res, Err(zerodds_idl::Error::DepthLimit { .. })),
        "depth=64 must NOT trigger DepthLimit, got {res:?}"
    );
}

/// Mutation-Test: 65 consecutive Annotations triggern den Cap.
/// Faengt `>` -> `==` und `>` -> `>=` Mutationen am Annotation-Limit.
#[test]
fn consecutive_annotations_just_over_cap_rejected() {
    let mut src = String::new();
    for _ in 0..65 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "65 annotations must trigger AnnotationLimit, got {res:?}"
    );
}

/// Mutation-Test: 64 consecutive Annotations duerfen NICHT triggern.
/// Faengt `>` -> `>=` Mutation am Annotation-Limit.
#[test]
fn consecutive_annotations_at_cap_accepted() {
    let mut src = String::new();
    for _ in 0..64 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        !matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "64 annotations must NOT trigger AnnotationLimit, got {res:?}"
    );
}

/// Mutation-Test: Semicolon resettet den Annotation-Counter.
/// Beweis: 200 `@final;` hintereinander darf NICHT in AnnotationLimit
/// laufen, weil jeder `;` den Counter resettet (auch wenn der Parser
/// dann bei Recognize wegen Syntax scheitert).
/// Faengt `==` -> `!=` Mutation auf der `;`-Reset-Branch (line 86).
#[test]
fn semicolon_resets_annotation_counter() {
    let mut src = String::new();
    for _ in 0..200 {
        src.push_str("@final; ");
    }
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    // Originalverhalten: pre-check passes (`;` resets), recognize
    // schlaegt syntaktisch fehl. Mutation `!=`: pre-check sammelt 200
    // `@`s ohne Reset und feuert AnnotationLimit ab dem 65sten.
    assert!(
        !matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "semicolons must reset annotation counter — got AnnotationLimit unexpectedly: {res:?}"
    );
}

/// Mutation-Test: `}`-Branch decrementiert depth korrekt; `@`-Branch
/// muss separat erreicht werden, nicht vom `}`-`!=`-Mutation
/// "swallowed" werden.
///
/// Beweis: 65 `@final` ohne Block-Strukturen MUSS in AnnotationLimit
/// laufen. Mit der Mutation `} else if p == "}"` -> `} else if p != "}"`
/// wuerde jeder `@` in den `}`-Branch fallen (weil `@` != `}`) und
/// `consecutive_at` waere immer 0, kein Cap-Trigger.
///
/// Eingabe ohne `{` `}` damit der `}`-Branch der einzige sein-koennte
/// der `@` "stiehlt" — der `;`-Branch greift erst nach `@`-Branch.
#[test]
fn close_brace_branch_does_not_swallow_at() {
    let mut src = String::new();
    for _ in 0..65 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    assert!(
        matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "65 @final must trigger AnnotationLimit (catches `}}`-branch swallowing `@`), got {res:?}"
    );
}

/// Mutation-Test: `@`-Branch tut tatsaechlich `consecutive_at += 1`,
/// nicht `*=` (wuerde 0 -> 0 -> 0 -> ... nie ueberlaufen).
/// Faengt `+=` -> `*=` Mutation.
#[test]
fn at_branch_increments_not_multiplies() {
    let mut src = String::new();
    for _ in 0..70 {
        src.push_str("@final ");
    }
    src.push_str("struct S { long x; };");
    let res = zerodds_idl::parse(&src, &ParserConfig::default());
    // Mit `*=`: `consecutive_at` startet 0, jedes `@` macht 0*1+0=0 oder
    // 0*1=0; nie > MAX. Original: jedes `@` ist +=1, faengt nach 65.
    assert!(
        matches!(res, Err(zerodds_idl::Error::AnnotationLimit { .. })),
        "increment-not-multiply: 70 @final must trigger AnnotationLimit, got {res:?}"
    );
}
