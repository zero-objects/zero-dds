// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! What the C++ and Java language bindings are made of — DDS-RPC 1.0
//! §7.11.1.3, §7.11.1.4, §7.11.1.5, §7.11.2.1 and §7.11.2.2.
//!
//! Two kinds of thing live here, and both are about *names*.
//!
//! The first is the exception mapping of §7.11.1.3: a table from
//! [`RemoteExceptionCode`] — the wire value a reply carries — to the C++
//! class a binding raises. §7.11.2.1, Java's own "Mapping of
//! Exceptions", has no body in the published document, and §7.11.2's
//! preamble says its sub clauses "describe the entities that are either
//! different or not described in the C++ language bindings" — so the
//! empty clause says the C++ table stands, and [`java_exception_class`]
//! is that table in the `org.omg.dds.rpc` package §7.11.2.2.1 names.
//!
//! The second is the class summaries of §7.11.1.4, §7.11.1.5 and
//! §7.11.2.2.1: the enumerated surface each binding has to provide.
//! They are here as lists a test can read rather than as prose, because
//! a class summary that lives only in a coverage document drifts from
//! the clause the moment either changes.
//!
//! ## What is not here
//!
//! The runtimes those classes name are not in this crate: the C++ one is
//! `runtime/cpp/include/dds/rpc.hpp` with its binding to the C API in
//! `runtime/cpp/include/dds/rpc/zerodds.hpp`, the Java one is
//! `runtime/java/src/main/java/org/omg/dds/rpc` (the interfaces of
//! §7.11.2.2.1) with the implementation classes in
//! `runtime/java/src/main/java/org/zerodds/ddsrpc`. The generated bindings in
//! `crates/lang-rules/src/emit/interface_{cpp,java}.rs` use them; the items of
//! `docs/spec-coverage/dds-rpc-1.0.md` record what each binding meets.

extern crate alloc;

use crate::common_types::RemoteExceptionCode;

/// What the exception table of §7.11.1.3 answers for one code.
///
/// Three states and not an `Option`, because "no exception" and "no
/// entry in the table" are different facts and a caller that folded them
/// would raise nothing in both cases — once because the call succeeded,
/// once because the code is one the table does not name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteExceptionClass {
    /// `REMOTE_EX_OK` — the table's "No exception".
    NoException,
    /// The class the table names.
    Class(&'static str),
    /// A code outside `RemoteExceptionCode_t`'s six.
    ///
    /// `RemoteExceptionCode::from_u32` rejects such a value before it can
    /// become an enum, so no code maps here; the state stays so a reader
    /// of the table can distinguish "outside the six" from "no
    /// exception" without folding the two.
    OutsideTable,
}

/// The C++ class §7.11.1.3's table raises for a remote exception code.
///
/// The table on p. 59: `REMOTE_EX_OK` is no exception,
/// `REMOTE_EX_UNSUPPORTED` is `dds::rpc::RemoteUnsupportedError`, and so
/// through the six values of `RemoteExceptionCode_t`.
#[must_use]
pub const fn cpp_exception_class(code: RemoteExceptionCode) -> RemoteExceptionClass {
    match code {
        RemoteExceptionCode::Ok => RemoteExceptionClass::NoException,
        RemoteExceptionCode::Unsupported => {
            RemoteExceptionClass::Class("dds::rpc::RemoteUnsupportedError")
        }
        RemoteExceptionCode::InvalidArgument => {
            RemoteExceptionClass::Class("dds::rpc::RemoteInvalidArgumentError")
        }
        RemoteExceptionCode::OutOfResources => {
            RemoteExceptionClass::Class("dds::rpc::RemoteOutOfResourcesError")
        }
        RemoteExceptionCode::UnknownOperation => {
            RemoteExceptionClass::Class("dds::rpc::RemoteUnknownOperationError")
        }
        RemoteExceptionCode::UnknownException => {
            RemoteExceptionClass::Class("dds::rpc::RemoteUnknownExceptionError")
        }
    }
}

/// The Java class the same table names — §7.11.2.1 through §7.11.2's
/// preamble, in the package §7.11.2.2.1 gives.
///
/// The C++ hierarchy of Figure 7.6 puts every remote exception under
/// `std::runtime_error` or `std::logic_error`, both of which are thrown
/// without being declared. The Java counterpart is an unchecked
/// exception, which is why
/// `crates/lang-rules/src/emit/interface_java.rs` declares no `throws`
/// clause on a generated method.
#[must_use]
pub const fn java_exception_class(code: RemoteExceptionCode) -> RemoteExceptionClass {
    match code {
        RemoteExceptionCode::Ok => RemoteExceptionClass::NoException,
        RemoteExceptionCode::Unsupported => {
            RemoteExceptionClass::Class("org.omg.dds.rpc.RemoteUnsupportedError")
        }
        RemoteExceptionCode::InvalidArgument => {
            RemoteExceptionClass::Class("org.omg.dds.rpc.RemoteInvalidArgumentError")
        }
        RemoteExceptionCode::OutOfResources => {
            RemoteExceptionClass::Class("org.omg.dds.rpc.RemoteOutOfResourcesError")
        }
        RemoteExceptionCode::UnknownOperation => {
            RemoteExceptionClass::Class("org.omg.dds.rpc.RemoteUnknownOperationError")
        }
        RemoteExceptionCode::UnknownException => {
            RemoteExceptionClass::Class("org.omg.dds.rpc.RemoteUnknownExceptionError")
        }
    }
}

/// The C++ namespaces of §7.11.1.4.1.
pub const CPP_NAMESPACES: [&str; 2] = ["dds", "dds::rpc"];

/// The classes §7.11.1.4.1 lists for the C++ request-reply style
/// binding, in the order the clause prints them.
pub const CPP_REQUEST_REPLY_CLASSES: [&str; 22] = [
    "RPCEntity",
    "Requester",
    "Replier",
    "ServiceProxy",
    "ListenerBase",
    "SimpleReplierListener",
    "ReplierListener",
    "SimpleRequesterListener",
    "RequesterListener",
    "RequesterParams",
    "ReplierParams",
    "future",
    "shared_future",
    "Sample",
    "SampleRef",
    "WriteSample",
    "WriteSampleRef",
    "LoanedSamples",
    "SharedSamples",
    "SampleIterator",
    "dds_type_traits",
    "dds_entity_traits",
];

/// The classes §7.11.1.5 lists for the C++ function-call style binding.
///
/// "`Service`, `ServiceEndpoint`, `Server`, `Client`, and
/// `ClientEndpoint` classes are specific to the function-call style
/// language binding."
pub const CPP_FUNCTION_CALL_CLASSES: [&str; 5] = [
    "Service",
    "ServiceEndpoint",
    "Server",
    "Client",
    "ClientEndpoint",
];

/// The parameter classes of the function-call style, which §7.4.2.3 names
/// and no summary clause lists.
///
/// "The ServiceParams, ClientParams, RequesterParams, and ReplierParams
/// classes (7.11.1.4 Summary of C++ Request-Reply Style Language Binding)
/// provide functions to specify the service name and the topic name
/// suffix." §7.11.1.4 holds the last two ([`CPP_REQUEST_REPLY_CLASSES`]);
/// the first two are the function-call counterparts of `ReplierParams` and
/// `RequesterParams`, one for a [`Service`](CPP_FUNCTION_CALL_CLASSES) and
/// one for a `Client`.
pub const CPP_FUNCTION_CALL_PARAMS_CLASSES: [&str; 2] = ["ServiceParams", "ClientParams"];

/// The Java packages of §7.11.2.2.1.
pub const JAVA_PACKAGES: [&str; 2] = ["org.omg.dds", "org.omg.dds.rpc"];

/// The types §7.11.2.2.1 lists for the Java request-reply style
/// binding, in the order the clause prints them.
pub const JAVA_REQUEST_REPLY_TYPES: [&str; 15] = [
    "Requester",
    "Replier",
    "ServiceProxy",
    "RPCEntity",
    "RPCRuntime",
    "SimpleReplierListener",
    "ReplierListener",
    "SimpleRequesterListener",
    "RequesterListener",
    "RequesterParams",
    "ReplierParams",
    "Future",
    "FutureCompletionListener",
    "Sample",
    "Sample.Iterator",
];

/// The types §7.11.2.3 gives the Java function-call style binding.
///
/// "Function-Call style language binding in Java shall define the same
/// entities as that of C++", so this is [`CPP_FUNCTION_CALL_CLASSES`]
/// and is stated as the same value rather than as a second list that
/// could drift from it.
pub const JAVA_FUNCTION_CALL_TYPES: [&str; 5] = CPP_FUNCTION_CALL_CLASSES;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    /// Every wire code has an answer in both tables, and the answer is
    /// the same *kind* in both: a code that raises in C++ raises in
    /// Java.
    #[test]
    fn both_exception_tables_are_total_over_the_wire_codes() {
        for raw in 0..=5u32 {
            let code = RemoteExceptionCode::from_u32(raw).unwrap();
            let cpp = cpp_exception_class(code);
            let java = java_exception_class(code);
            match (cpp, java) {
                (RemoteExceptionClass::Class(cpp), RemoteExceptionClass::Class(java)) => {
                    assert!(cpp.starts_with("dds::rpc::"), "{cpp}");
                    assert!(java.starts_with("org.omg.dds.rpc."), "{java}");
                    assert_eq!(
                        cpp.rsplit("::").next(),
                        java.rsplit('.').next(),
                        "the two bindings name one class differently for {raw}"
                    );
                }
                (RemoteExceptionClass::NoException, RemoteExceptionClass::NoException)
                | (RemoteExceptionClass::OutsideTable, RemoteExceptionClass::OutsideTable) => {}
                other => panic!("{raw} answers differently in the two bindings: {other:?}"),
            }
        }
    }

    /// §7.11.1.3's table: `REMOTE_EX_OK` is "No exception" and the five
    /// others each name one class.
    #[test]
    fn the_table_of_seven_eleven_one_three_is_the_published_one() {
        assert_eq!(
            cpp_exception_class(RemoteExceptionCode::Ok),
            RemoteExceptionClass::NoException
        );
        for (code, class) in [
            (
                RemoteExceptionCode::Unsupported,
                "dds::rpc::RemoteUnsupportedError",
            ),
            (
                RemoteExceptionCode::InvalidArgument,
                "dds::rpc::RemoteInvalidArgumentError",
            ),
            (
                RemoteExceptionCode::OutOfResources,
                "dds::rpc::RemoteOutOfResourcesError",
            ),
            (
                RemoteExceptionCode::UnknownOperation,
                "dds::rpc::RemoteUnknownOperationError",
            ),
            (
                RemoteExceptionCode::UnknownException,
                "dds::rpc::RemoteUnknownExceptionError",
            ),
        ] {
            assert_eq!(
                cpp_exception_class(code),
                RemoteExceptionClass::Class(class)
            );
        }
    }

    /// Every code of §7.5.1.1.1's six has a row; `from_u32` rejects a
    /// seventh before it can reach the table.
    #[test]
    fn the_exception_table_covers_the_six_spec_codes() {
        assert!(RemoteExceptionCode::from_u32(6).is_err());
    }

    /// The class summaries hold every name once.
    #[test]
    fn the_class_summaries_have_no_repeated_name() {
        for list in [
            CPP_REQUEST_REPLY_CLASSES.as_slice(),
            CPP_FUNCTION_CALL_CLASSES.as_slice(),
            CPP_FUNCTION_CALL_PARAMS_CLASSES.as_slice(),
            JAVA_REQUEST_REPLY_TYPES.as_slice(),
        ] {
            let all: BTreeSet<&str> = CPP_REQUEST_REPLY_CLASSES
                .iter()
                .chain(CPP_FUNCTION_CALL_CLASSES.iter())
                .chain(CPP_FUNCTION_CALL_PARAMS_CLASSES.iter())
                .copied()
                .collect();
            assert_eq!(all.len(), 22 + 5 + 2, "no name is in two lists");
            let unique: BTreeSet<&str> = list.iter().copied().collect();
            assert_eq!(unique.len(), list.len(), "{list:?}");
        }
        assert_eq!(CPP_NAMESPACES.len(), 2);
        assert_eq!(JAVA_PACKAGES.len(), 2);
    }

    /// §7.11.2.3: the Java function-call binding defines the same
    /// entities as C++'s.
    #[test]
    fn the_java_function_call_types_are_the_cpp_ones() {
        assert_eq!(JAVA_FUNCTION_CALL_TYPES, CPP_FUNCTION_CALL_CLASSES);
    }

    /// The names the generated C++ header writes are all in the class
    /// summaries — a generated header that named a class the binding
    /// does not have would compile nowhere and be found by a C++
    /// compiler rather than here.
    #[test]
    fn the_generated_bindings_name_only_summarised_classes() {
        for named in ["Requester", "Replier", "future"] {
            assert!(
                CPP_REQUEST_REPLY_CLASSES.contains(&named),
                "{named} is not in §7.11.1.4.1's list"
            );
        }
        for named in ["ClientEndpoint", "ServiceEndpoint"] {
            assert!(
                CPP_FUNCTION_CALL_CLASSES.contains(&named),
                "{named} is not in §7.11.1.5's list"
            );
        }
        for named in ["Requester", "Replier", "Future", "RPCRuntime"] {
            assert!(
                JAVA_REQUEST_REPLY_TYPES.contains(&named),
                "{named} is not in §7.11.2.2.1's list"
            );
        }
    }
}
