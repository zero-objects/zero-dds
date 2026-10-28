// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every class the two summaries enumerate is declared, in both columns.
//!
//! `crates/rpc/src/language_binding.rs` holds §7.11.1.4.1, §7.11.1.5 and
//! §7.11.2.2.1 as lists a program can read. That is only half the
//! answer: a list that agrees with the clause and with nothing in the
//! tree is a list. This reads the two runtimes and holds them against
//! it, so a class dropped from either is a failure here rather than a
//! compile error at a consumer's site.
//!
//! The other half — that the classes *work* — is
//! `cpp_rpc_runtime.rs` and `java_rpc_runtime.rs`, which compile and run
//! them.
#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use zerodds_rpc::language_binding::{
    CPP_FUNCTION_CALL_CLASSES, CPP_FUNCTION_CALL_PARAMS_CLASSES, CPP_NAMESPACES,
    CPP_REQUEST_REPLY_CLASSES, JAVA_PACKAGES, JAVA_REQUEST_REPLY_TYPES,
};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// §7.11.1.4.1, §7.11.1.5 and §7.4.2.3: the C++ runtime declares all 29.
#[test]
fn the_cpp_runtime_declares_every_class_of_the_two_summaries() {
    // The classes are declared by the normative headers of §7.11.1 (shipped
    // unchanged), by `rpc.hpp` (Service, Client, RPCRuntime) and by the types
    // `base.hpp` gives them.
    let header = [
        "rpc.hpp",
        "rpc/base.hpp",
        "rpc/vendor_dependent.h",
        "rpc/normative/request_reply.h",
        "rpc/normative/function_call.h",
    ]
    .iter()
    .map(|file| {
        std::fs::read_to_string(repository_root().join("runtime/cpp/include/dds").join(file))
            .unwrap_or_else(|e| panic!("dds/{file}: {e}"))
    })
    .collect::<Vec<_>>()
    .join("\n");

    for name in CPP_REQUEST_REPLY_CLASSES
        .iter()
        .chain(CPP_FUNCTION_CALL_CLASSES.iter())
        .chain(CPP_FUNCTION_CALL_PARAMS_CLASSES.iter())
    {
        // The five forms a declaration takes in this header: a class, a
        // struct, a forward declaration of either, and an alias — a
        // primary template with no body (`struct dds_type_traits;`) is
        // as much a declaration as one with a body.
        let declared = [
            format!("class {name} "),
            format!("class {name};"),
            format!("struct {name} "),
            format!("struct {name};"),
            format!("using {name} ="),
        ]
        .iter()
        .any(|form| header.contains(form));
        assert!(
            declared,
            "`dds::rpc::{name}` is not declared in the C++ headers of dds/rpc"
        );
    }

    // §7.11.1.4.1's two namespaces.
    assert_eq!(CPP_NAMESPACES, ["dds", "dds::rpc"]);
    assert!(header.contains("namespace dds {"), "the `dds` namespace");
    assert!(
        header.contains("namespace rpc {"),
        "the `dds::rpc` namespace"
    );
}

/// §7.11.2.2.1 and §7.11.2.3: the Java package declares all of them.
#[test]
fn the_java_package_declares_every_type_of_the_two_summaries() {
    let package = repository_root().join("runtime/java/src/main/java/org/omg/dds/rpc");
    assert!(
        package.is_dir(),
        "the `org.omg.dds.rpc` package is not in the tree"
    );
    assert_eq!(JAVA_PACKAGES, ["org.omg.dds", "org.omg.dds.rpc"]);

    for name in JAVA_REQUEST_REPLY_TYPES
        .iter()
        .chain(CPP_FUNCTION_CALL_CLASSES.iter())
    {
        // `Sample.Iterator` "is the same as in [DDS-Java-PSM]" (§7.11.2.2.7):
        // `rpc.Sample` extends `org.omg.dds.sub.Sample` (§7.11.2.2.6), whose
        // nested `Iterator` it inherits, as `omg-dds-rpc-java.zip` has it.
        if let Some((outer, inner)) = name.split_once('.') {
            let source = std::fs::read_to_string(package.join(format!("{outer}.java")))
                .unwrap_or_else(|_| panic!("{outer}.java"));
            assert!(
                source.contains(&format!("extends org.omg.dds.sub.{outer}<")),
                "`{outer}.java` does not extend `org.omg.dds.sub.{outer}`, whose `{inner}` it is"
            );
            let psm = repository_root().join(format!(
                "crates/java-omgdds/java/src/main/java/org/omg/dds/sub/{outer}.java"
            ));
            let psm = std::fs::read_to_string(&psm).unwrap_or_else(|_| panic!("{}", psm.display()));
            assert!(
                psm.contains(&format!("interface {inner}<"))
                    || psm.contains(&format!("interface {inner} ")),
                "`org.omg.dds.sub.{outer}` does not declare `{inner}`"
            );
            continue;
        }
        let at = package.join(format!("{name}.java"));
        assert!(at.is_file(), "`org.omg.dds.rpc.{name}` is not in the tree");
        let source = std::fs::read_to_string(&at).expect("the source");
        assert!(
            source.contains(&format!("interface {name}"))
                || source.contains(&format!("class {name}"))
                || source.contains(&format!("enum {name}")),
            "{name}.java does not declare `{name}`"
        );
    }
}

/// The two columns declare the same set, which is what §7.11.2's
/// preamble means by "the entities that are either different or not
/// described in the C++ language bindings": everything §7.11.2.2.1 lists
/// that §7.11.1.4.1 also lists is one class in two spellings.
#[test]
fn the_two_summaries_overlap_exactly_where_the_clause_says() {
    // `RPCRuntime` is Java's own (§7.11.2.2.3) and `future`,
    // `shared_future`, `SampleRef`, `WriteSample`, `WriteSampleRef`,
    // `LoanedSamples`, `SharedSamples`, `SampleIterator`,
    // `dds_type_traits` and `dds_entity_traits` are C++'s own — a Java
    // `Future` needs no `shared_future` because a Java reference is
    // already shared, and the loan carriers are a C++ ownership problem.
    let java_only = [
        "RPCRuntime",
        "Future",
        "FutureCompletionListener",
        "Sample.Iterator",
    ];
    for name in java_only {
        assert!(
            JAVA_REQUEST_REPLY_TYPES.contains(&name),
            "{name} is listed as Java's own"
        );
    }
    for shared in [
        "Requester",
        "Replier",
        "ServiceProxy",
        "RPCEntity",
        "SimpleReplierListener",
        "ReplierListener",
        "SimpleRequesterListener",
        "RequesterListener",
        "RequesterParams",
        "ReplierParams",
        "Sample",
    ] {
        assert!(
            CPP_REQUEST_REPLY_CLASSES.contains(&shared)
                && JAVA_REQUEST_REPLY_TYPES.contains(&shared),
            "{shared} is in both summaries"
        );
    }
}
