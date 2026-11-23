// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A `.cdl` file is OMG CIDL: CORBA 3.3 Part 3 §8.1, "A source file
//! containing specifications written in OMG CIDL must have a “.cdl”
//! extension."
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::process::Command;

const BANK: &str = "module Bank {
    interface Teller { void serve(); };
    interface Log {};
    valuetype AccountKey { public string accno; };
    component Account { provides Teller teller; provides Log log; attribute long balance; };
    home AccountHome manages Account primarykey AccountKey {
        factory open(in string accno, in long balance);
    };
    component Kiosk { provides Teller desk; };
    home KioskHome manages Kiosk {};
};
module Store {
    abstract storagetype Ledger {
        state string accno; state long balance; long total() const; void reset();
    };
    abstract storagehome LedgerHome of Ledger { key accno; factory create(accno, balance); };
};
module Impl {
    composition entity AccountImpl {
        home executor AccountHomeImpl {
            implements Bank::AccountHome;
            bindsTo Store::LedgerHome;
            manages AccountExec {
                segment Audit { provides facet log; }
                delegatesTo storage (balance: balance)
            };
            delegatesTo abstract storagehome (open: create);
        };
    };
";

fn run(args: &[&str], name: &str, text: &str) -> (i32, String) {
    // One directory per call: under `cargo test` the tests run as threads of
    // one process, and a directory named after the process and the file
    // alone was removed by one test while another still read its file.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "zerodds-idlc-cidl-{}-{call}-{}",
        std::process::id(),
        name.replace('.', "-")
    ));
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    let file = dir.join(name);
    std::fs::write(&file, text).expect("the file");
    let output = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
        .args(args)
        .arg(&file)
        .output()
        .expect("the binary runs");
    let _ = std::fs::remove_dir_all(&dir);
    let mut both = String::from_utf8_lossy(&output.stdout).into_owned();
    both.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.code().expect("an exit code"), both)
}

#[test]
fn corba_p3_8_1_a_cdl_file_is_checked_as_cidl() {
    let valid = format!("{BANK}}};\n");
    let (code, out) = run(&["check"], "bank.cdl", &valid);
    assert_eq!(code, 0, "{out}");
    // The same text as IDL: `composition` is no IDL declaration.
    let (code, out) = run(&["check"], "bank.idl", &valid);
    assert_eq!(code, 1, "{out}");
    // A composition rule is a diagnostic with its code: an entity
    // composition over a home without a primary key (Table 8.5).
    let invalid = format!(
        "{BANK}composition entity KioskImpl {{ home executor H {{ implements Bank::KioskHome;
            manages E; }}; }};\n}};\n"
    );
    let (code, out) = run(&["check"], "kiosk.cdl", &invalid);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("IDL0024"), "{out}");
    assert!(out.contains("Table 8.5"), "{out}");
}

#[test]
fn corba_p3_8_11_dump_cidl_shows_the_compositions_and_their_segment_ids() {
    let (code, out) = run(&["dump-cidl"], "bank.cdl", &format!("{BANK}}};\n"));
    assert_eq!(code, 0, "{out}");
    for line in [
        "abstract storagehome Store::LedgerHome of Store::Ledger",
        "  operation total const",
        "  operation reset",
        "  key accno(accno)",
        "  factory create(accno, balance)",
        "composition entity Impl::AccountImpl",
        "  home executor AccountHomeImpl implements Bank::AccountHome",
        "    bindsTo Store::LedgerHome",
        "    manages AccountExec (segmented)",
        "      segment 0 (component segment)",
        "      segment 1 Audit: log",
        "      delegatesTo storage balance: balance",
        "    delegatesTo abstract storagehome (open: create)",
    ] {
        assert!(
            out.lines().any(|l| l == line),
            "no line {line:?} in:\n{out}"
        );
    }
}

#[test]
fn corba_p3_8_1_cidl_is_refused_where_idl_is_read_and_dump_cidl_reads_only_cidl() {
    let valid = format!("{BANK}}};\n");
    for command in [&["generate", "--lang", "rust"][..], &["lint"], &["format"]] {
        let (code, out) = run(command, "bank.cdl", &valid);
        assert_eq!(code, 1, "{command:?}: {out}");
        assert!(out.contains("CIDL file (.cdl)"), "{command:?}: {out}");
    }
    let (code, out) = run(&["dump-cidl"], "plain.idl", "struct S { long x; };\n");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("`dump-cidl` reads a CIDL file"), "{out}");
}

/// A home executor body out of the order of production (8), a
/// delegation where its production does not place it, and an empty
/// executor body fail `check`, and `dump-cidl` shows none of them as a
/// member of the composition.
#[test]
fn corba_p3_8_6_a_member_out_of_its_place_fails_the_check() {
    let composition = |body: &str| {
        format!("{BANK}composition entity Probe {{ home executor P {{ {body} }}; }};\n}};\n")
    };
    for (body, message) in [
        (
            "implements Bank::AccountHome; bindsTo Store::LedgerHome; manages EA;
             delegatesTo storage (balance: nosuchmember);",
            "`delegatesTo storage` does not belong in the home executor body",
        ),
        (
            "implements Bank::AccountHome; bindsTo Store::LedgerHome; manages EB;
             delegatesTo home (close_all: wipe);",
            "`delegatesTo home` does not belong in the home executor body",
        ),
        (
            "manages EC1; bindsTo Bank::Teller; implements Bank::AccountHome;
             bindsTo Store::LedgerHome; manages EC2;",
            "`bindsTo` after `manages`",
        ),
        (
            "implements Bank::AccountHome; manages ED { delegatesTo executor (bogus) };",
            "`delegatesTo executor` does not belong in the executor body",
        ),
        ("implements Bank::AccountHome;", "has no `manages`"),
        (
            "implements Bank::AccountHome; manages EG { };",
            "an executor body holds at least one",
        ),
    ] {
        let source = composition(body);
        let (code, out) = run(&["check"], "probe.cdl", &source);
        assert_eq!(code, 1, "{body}: {out}");
        assert!(out.contains(message), "{body}: {out}");
        let (code, dump) = run(&["dump-cidl"], "probe.cdl", &source);
        assert_eq!(code, 1, "{body}: {dump}");
        for absent in [
            "delegatesTo abstract storagehome (close_all: wipe)",
            "bindsTo Bank::Teller",
        ] {
            assert!(
                !dump.lines().any(|l| l.trim() == absent),
                "{body}: {absent} in\n{dump}"
            );
        }
    }
    // A proxy home takes only `delegatesTo home`.
    let proxy = format!(
        "{BANK}composition session ProxyProbe {{ home executor P {{
            implements Bank::KioskHome; manages E; }};
            proxy home PE {{ delegatesTo storage (balance: zzz); }}; }};\n}};\n"
    );
    let (code, out) = run(&["check"], "proxy.cdl", &proxy);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("`delegatesTo storage` does not belong in a proxy home"),
        "{out}"
    );
}
