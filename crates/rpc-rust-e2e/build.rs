// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Generates the Rust DDS-RPC 1.0 binding this crate's tests run.
//!
//! The generator runs at build time and its output is compiled with
//! the rest of the crate — the point, exactly as
//! `crates/wire-conformance/build.rs` puts it: a name comparison
//! cannot see a signature that does not match, and nothing else in the
//! tree compiles what `zerodds_lang_rules::generate::source` writes for
//! an `interface`.
//!
//! Two sources: the task's own `Calculator` — an exception, an
//! attribute, a `void` operation and a `raises` clause, so every
//! construct DDS-RPC 1.0 §7.5.1.1 and §7.5.1.1.3 name is in the file —
//! and a small interface hierarchy, so `Ast::all_operations_of`'s
//! counterpart on the client side (`crate::emit::interface_rust`'s base
//! composition) has something to compile against.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use zerodds_ast::Ast;
use zerodds_ast_layout::key::{Encoding, Endianness, LayoutKey};
use zerodds_lang_cst::Language;

const CALCULATOR: &str = "
module demo {
  exception DivByZero { string msg; };
  @DDSService interface Calculator {
    long add(in long a, in long b);
    double div(in double a, in double b) raises (DivByZero);
    void reset();
    void swap(inout long v);
    attribute long memory;
  };
};
";

/// Members of eight octets behind members of one, four and eight: where
/// classic CDR (XCDR1) and XCDR2 put the octets differently
/// (`tests/wire_bytes_match_columns.rs`).
const WIDE: &str = "
module wide {
  @DDSService interface W {
    double scale(in double d, in long long n, in unsigned long long u);
    unsigned long long mix(in octet o, in unsigned long long u);
  };
};
";

const HIERARCHY: &str = "
module hierarchy {
  @DDSService interface Base {
    long ping(in long x);
  };
  @DDSService interface Derived : Base {
    long twice(in long x);
  };
};
";

/// Nested `@appendable` and `@mutable` members in the operations of an
/// interface: what the XCDR1 (classic CDR) codec of the Basic mapping has
/// to frame without a DHEADER (XTypes 1.3 §7.4.1.1) and as a parameter
/// list (§7.4.1.2).
const NESTED: &str = "
module nested {
  @appendable struct Appended { long a; double b; };
  @mutable struct Mutated { long m; @id(7) long n; };
  @DDSService interface Store {
    long put(in Appended a, in Mutated m);
    Mutated get();
  };
};
";

/// A service defined by a pair of types that carry their own header
/// (DDS-RPC 1.0 §7.3.1.4, Basic Service Mapping), over the common types
/// of §7.5.1.1.1.
const PAIR: &str = "
module dds {
  typedef octet GuidPrefix_t[12];
  struct EntityId_t { octet entityKey[3]; octet entityKind; };
  struct GUID_t { GuidPrefix_t guidPrefix; EntityId_t entityId; };
  struct SequenceNumber_t { long high; unsigned long low; };
  struct SampleIdentity { GUID_t writer_guid; SequenceNumber_t sequence_number; };
  module rpc {
    enum RemoteExceptionCode_t {
      REMOTE_EX_OK, REMOTE_EX_UNSUPPORTED, REMOTE_EX_INVALID_ARGUMENT,
      REMOTE_EX_OUT_OF_RESOURCES, REMOTE_EX_UNKNOWN_OPERATION, REMOTE_EX_UNKNOWN_EXCEPTION
    };
    // §7.5.1.1.1 writes `InstanceName instanceName;`, which IDL refuses:
    // the two names differ only in case within one scope.
    struct RequestHeader { SampleIdentity requestId; string<255> instanceName; };
    struct ReplyHeader { SampleIdentity relatedRequestId; RemoteExceptionCode_t remoteEx; };
  };
};
module robot {
  @RPCRequestType struct MoveRequest { dds::rpc::RequestHeader header; long x; double speed; };
  @RPCReplyType struct MoveReply { dds::rpc::ReplyHeader header; boolean ok; };
};
";

/// The Enhanced Service Mapping of DDS-RPC 1.0 §7.5.1.2: an exception, an
/// `inout` parameter, an operation with nothing to carry, an operation
/// with `@Mutable`, and a base interface.
const ENHANCED: &str = "
module enh {
  exception TooBig { long limit; };
  @DDSService interface Calc {
    long add(in long a, in long b);
    long scale(in long k) raises (TooBig);
    void swap(inout long v);
    void ping();
  };
  @DDSService interface Fast {
    @Mutable long square(in long x);
  };
  @DDSService interface Base2 {
    long ident();
  };
  @DDSService interface Derived2 : Base2 {
    long more();
  };
};
";

/// The Enhanced Service Mapping with an operation that has nothing to
/// carry as an empty `@Empty` structure (DDS-RPC 1.0 §7.5.1.2.1.3).
const EMPTY: &str = "
module emp {
  interface Pinger {
    void ping();
    long count();
  };
};
";

/// Two versions of one Enhanced interface, `Iold` and `Inew` of DDS-RPC 1.0
/// §7.7.2, generated into two Rust modules with the same IDL names: an
/// operation only the old one has, one only the new one has, a `@Mutable`
/// operation whose parameters are reordered, and an exception the new one
/// adds to an operation.
const EVOLUTION_OLD: &str = "
module evo {
  exception Overflow { long limit; };
  @DDSService interface Svc {
    long add(in long a, in long b);
    long scale(in long k) raises (Overflow);
    @Mutable long mix(in long x, in long y);
    long legacy();
  };
};
";

/// The evolved interface of [`EVOLUTION_OLD`].
const EVOLUTION_NEW: &str = "
module evo {
  exception Overflow { long limit; };
  exception Underflow { long limit; };
  @DDSService interface Svc {
    long add(in long a, in long b);
    long scale(in long k) raises (Overflow, Underflow);
    @Mutable long mix(in long y, in long x);
    long extra();
  };
};
";

/// [`EVOLUTION_OLD`] with the type of a parameter changed: not an evolution
/// DDS-RPC 1.0 §7.7.2 allows (§7.7.2.5.3), so a peer of the old version must
/// not be matched with it.
const EVOLUTION_CHANGED: &str = "
module evo {
  exception Overflow { long limit; };
  @DDSService interface Svc {
    long add(in string a, in long b);
    long scale(in long k) raises (Overflow);
    @Mutable long mix(in long x, in long y);
    long legacy();
  };
};
";

/// [`EVOLUTION_OLD`] with the return type of an operation changed to one the
/// old type is not assignable to (§7.7.2.5.5).
const EVOLUTION_RETURN: &str = "
module evo {
  exception Overflow { long limit; };
  @DDSService interface Svc {
    long add(in long a, in long b);
    long long scale(in long k) raises (Overflow);
    @Mutable long mix(in long x, in long y);
    long legacy();
  };
};
";

/// A structure parameter of an interface, before and after a member was
/// appended to it: a change of the type of a parameter that XTypes 1.3
/// §7.2.4.4 allows (DDS-RPC 1.0 §7.7.2.5.3).
const EVOLUTION_POSE_OLD: &str = "
module shapes {
  @mutable struct Pose { long x; };
  @DDSService interface Svc {
    long length(in Pose p);
    Pose origin();
  };
};
";

/// The interface of [`EVOLUTION_POSE_OLD`] with a member added to `Pose`.
const EVOLUTION_POSE_NEW: &str = "
module shapes {
  @mutable struct Pose { long x; long y; };
  @DDSService interface Svc {
    long length(in Pose p);
    Pose origin();
  };
};
";

/// Three versions of one Basic interface, DDS-RPC 1.0 §7.7.1: the Basic
/// Service Mapping allows operations to be added and removed and nothing
/// else (§7.7.1.3), so the three differ only in the set of operations.
/// `BASIC_FEWER` has fewer operations than the other two.
const BASIC_FEWER: &str = "
module evo {
  @DDSService interface Svc {
    long add(in long a, in long b);
  };
};
";

/// The Basic version with an operation the evolved one no longer has.
const BASIC_OLD: &str = "
module evo {
  @DDSService interface Svc {
    long add(in long a, in long b);
    long legacy();
  };
};
";

/// The evolved Basic version of [`BASIC_OLD`]: `legacy` is gone, `extra` is
/// new, and the order of the remaining operations is another one
/// (§7.7.1.2).
const BASIC_NEW: &str = "
module evo {
  @DDSService interface Svc {
    long extra();
    long add(in long a, in long b);
  };
};
";

fn model(source: &str, file: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "{file} does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), file);
    assert!(
        lowered.diagnostics.iter().all(|d| !d.is_error()),
        "{file} does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.broken.is_empty(),
        "{file}: unresolved refs: {:?}",
        report.broken
    );
    ast
}

fn write_generated(source: &str, file: &str, out_name: &str, out_dir: &std::path::Path) {
    let ast = model(source, file);
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let text = zerodds_lang_rules::generate::source(&ast, Language::Rust, key)
        .unwrap_or_else(|error| panic!("{file}: {error}"));
    std::fs::write(out_dir.join(out_name), text).expect("write the generated file");
}

fn write_generated_enhanced(
    source: &str,
    file: &str,
    out_name: &str,
    out_dir: &std::path::Path,
    mapping: zerodds_lang_rules::generate::ServiceMapping,
) {
    let ast = model(source, file);
    let key = LayoutKey::data(Encoding::PlainCdr2, Endianness::Little);
    let text = zerodds_lang_rules::generate::source_with_mapping(
        &ast,
        Language::Rust,
        key,
        zerodds_lang_rules::emit::encode::CodecOptions::default(),
        mapping,
    )
    .unwrap_or_else(|error| panic!("{file}: {error}"));
    std::fs::write(out_dir.join(out_name), text).expect("write the generated file");
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    write_generated(CALCULATOR, "calculator.idl", "calculator.rs", &out_dir);
    write_generated(HIERARCHY, "hierarchy.idl", "hierarchy.rs", &out_dir);
    write_generated(WIDE, "wide.idl", "wide.rs", &out_dir);
    write_generated(NESTED, "nested.idl", "nested.rs", &out_dir);
    write_generated(PAIR, "pair.idl", "pair.rs", &out_dir);
    write_generated_enhanced(
        ENHANCED,
        "enhanced.idl",
        "enhanced.rs",
        &out_dir,
        zerodds_lang_rules::generate::ServiceMapping::Enhanced,
    );
    write_generated_enhanced(
        EVOLUTION_OLD,
        "evolution_old.idl",
        "evolution_old.rs",
        &out_dir,
        zerodds_lang_rules::generate::ServiceMapping::Enhanced,
    );
    write_generated_enhanced(
        EVOLUTION_NEW,
        "evolution_new.idl",
        "evolution_new.rs",
        &out_dir,
        zerodds_lang_rules::generate::ServiceMapping::Enhanced,
    );
    write_generated_enhanced(
        EVOLUTION_CHANGED,
        "evolution_changed.idl",
        "evolution_changed.rs",
        &out_dir,
        zerodds_lang_rules::generate::ServiceMapping::Enhanced,
    );
    for (source, name) in [
        (EVOLUTION_RETURN, "evolution_return"),
        (EVOLUTION_POSE_OLD, "evolution_pose_old"),
        (EVOLUTION_POSE_NEW, "evolution_pose_new"),
    ] {
        write_generated_enhanced(
            source,
            &format!("{name}.idl"),
            &format!("{name}.rs"),
            &out_dir,
            zerodds_lang_rules::generate::ServiceMapping::Enhanced,
        );
    }
    write_generated(BASIC_FEWER, "basic_fewer.idl", "basic_fewer.rs", &out_dir);
    write_generated(BASIC_OLD, "basic_old.idl", "basic_old.rs", &out_dir);
    write_generated(BASIC_NEW, "basic_new.idl", "basic_new.rs", &out_dir);
    write_generated_enhanced(
        EMPTY,
        "empty.idl",
        "empty.rs",
        &out_dir,
        zerodds_lang_rules::generate::ServiceMapping::EnhancedEmptyStructures,
    );
}
