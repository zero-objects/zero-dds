// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL `interface` to the three RPC language bindings, through every
//! layer.
//!
//! Parse, lower, resolve, read the seam, materialise, render. The layer
//! that used to be missing is the last two: `adapter::ast` put the
//! interface into the graph and `rule::{cpp,java,typescript}` each
//! matched it, and no materialiser read the result — so every assertion
//! here is over rendered text rather than over a tree, because a tree
//! that renders to nothing is what the gap looked like.
//!
//! - DDS-RPC 1.0 §7.5.1.1.3-8 — the implied operations and the topic
//!   types.
//! - DDS-RPC 1.0 §7.11.1.1.1, §7.11.1.1.2, §7.11.1.4, §7.11.1.5 — C++.
//! - DDS-RPC 1.0 §7.11.2.1, §7.11.2.2, §7.11.2.3 — Java.
//! - DDS-TS 1.0 Ch11 — TypeScript.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::Ast;
use zerodds_lang_cst::{Language, cpp};
use zerodds_lang_rules::adapter::interface::{self, BindingError, InterfaceError, Origin};
use zerodds_lang_rules::emit::{interface_cpp, interface_java, interface_typescript};
use zerodds_lang_rules::{ModelGraph, apply, rule};

/// Parse, lower and resolve — the frontend every binding reads through.
fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(
        parsed.diagnostics().is_empty(),
        "the source does not parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "the source does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

/// Like [`model`], for a source the frontend lowers with error
/// diagnostics (a construct the service grammar removed); only a parse
/// failure of the whole source is still fatal.
fn model_lenient(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("resolve");
    ast
}

/// The error the seam answers for `idl`.
fn refused(idl: &str) -> InterfaceError {
    interface::services(&model_lenient(idl)).expect_err("the seam refuses the source")
}

/// The C++ RPC binding of one IDL source, rendered.
fn cpp(idl: &str) -> String {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let ast = model(idl);
    interface_cpp::cpp_interface_items(&ast)
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(zerodds_lang_render::cpp::decl)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The Java RPC binding of one IDL source, rendered.
fn java(idl: &str) -> String {
    let ast = model(idl);
    interface_java::java_interface_items(&ast)
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(zerodds_lang_render::java::type_decl)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The TypeScript Operations-Profile output of one IDL source, rendered.
fn typescript(idl: &str) -> String {
    let ast = model(idl);
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::catalogue(Language::TypeScript)).expect("apply");
    interface_typescript::ts_interface_items(&ast, &applied)
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(zerodds_lang_render::typescript::decl)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The interface every binding test below reads.
const CALCULATOR: &str = "
    module robot {
      exception Overflow { long code; };
      struct Pose { long x; long y; };
      interface Base { long ping(); };
      interface Calculator : Base {
        readonly attribute long total;
        attribute string label;
        long add(in long a, in long b) raises (Overflow);
        long divmod(in long a, in long b, out long remainder);
        Pose locate(in string room);
        void reset();
      };
    };
";

// -- the seam ----------------------------------------------------------

/// The seam reads what OMG IDL 4.2 §7.4.3 declares: the operations, each
/// parameter's direction, the return, the `raises` list, the attributes
/// and the inheritance chain.
#[test]
fn the_seam_reads_every_part_of_an_interface() {
    let ast = model(CALCULATOR);
    let services = interface::services(&ast).expect("services");
    assert_eq!(services.len(), 2, "two interfaces are declared");

    let calculator = services
        .iter()
        .find(|service| service.name == "Calculator")
        .expect("Calculator");
    assert_eq!(
        calculator
            .bases
            .iter()
            .map(|base| base.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Base"],
        "the inheritance chain, in declaration order"
    );
    assert_eq!(
        calculator
            .operations
            .iter()
            .map(|operation| operation.name.as_str())
            .collect::<Vec<_>>(),
        vec!["add", "divmod", "locate", "reset"]
    );

    let add = &calculator.operations[0];
    assert_eq!(
        add.raises
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Overflow"]
    );
    assert!(add.returns.is_some(), "`long add(...)` returns");
    assert!(!add.has_outputs(), "every parameter of `add` is `in`");

    let divmod = &calculator.operations[1];
    assert!(divmod.has_outputs(), "`out long remainder`");
    assert_eq!(divmod.inputs().count(), 2);
    assert_eq!(divmod.outputs().count(), 1);

    let reset = &calculator.operations[3];
    assert!(reset.returns.is_none());

    assert_eq!(
        calculator
            .attributes
            .iter()
            .map(|attribute| (attribute.name.as_str(), attribute.readonly))
            .collect::<Vec<_>>(),
        vec![("total", true), ("label", false)]
    );
}

/// DDS-RPC 1.0 §7.5.1.1.3 rule 1: `get_attribute_<name>()` and
/// `set_attribute_<name>(<type>)`. A `readonly` attribute implies the
/// getter alone.
#[test]
fn an_attribute_implies_the_operations_of_seven_five_one_one_three() {
    let ast = model(CALCULATOR);
    let services = interface::services(&ast).expect("services");
    let calculator = services
        .iter()
        .find(|service| service.name == "Calculator")
        .expect("Calculator");

    let implied: Vec<(String, Origin)> = calculator
        .all_operations()
        .into_iter()
        .filter(|operation| operation.origin != Origin::Declared)
        .map(|operation| (operation.name.clone(), operation.origin))
        .collect();
    assert_eq!(
        implied,
        vec![
            ("get_attribute_total".to_string(), Origin::AttributeGetter),
            ("get_attribute_label".to_string(), Origin::AttributeGetter),
            ("set_attribute_label".to_string(), Origin::AttributeSetter),
        ],
        "`total` is readonly, so it implies no setter"
    );

    let all = calculator.all_operations();
    let setter = all
        .iter()
        .find(|operation| operation.name == "set_attribute_label")
        .expect("the setter");
    assert!(setter.returns.is_none(), "`void set_<name>(<type>)`");
    assert_eq!(setter.params.len(), 1);
}

/// `all_operations_flattened`'s own dedup guard, exercised directly:
/// a getter and a setter share one `node_id` (`Service::all_operations`
/// gives both `Operation::node_id: Some(attribute.node_id)`, since both
/// answer for the one `attribute` declaration that implies them) —
/// deduping by `node_id` alone, which an earlier version of this
/// function did, took the setter for a diamond-repeat of the getter and
/// silently dropped it for every non-`readonly` attribute reached
/// through `all_operations_flattened`, in every one of the eleven
/// language columns that read it. A diamond repeat is the case the
/// dedup exists for at all, so both have to hold at once: the shared
/// base's operation reached through two paths collapses to one, and a
/// writable attribute's getter and setter both survive.
#[test]
fn all_operations_flattened_keeps_a_non_readonly_attributes_setter() {
    const DIAMOND_WITH_ATTRIBUTE: &str = "
        interface Top { void t(); attribute long shared; };
        interface Left : Top { void l(); };
        interface Right : Top { void r(); };
        interface Bottom : Left, Right {
          void b();
          attribute long memory;
        };
    ";
    let ast = model(DIAMOND_WITH_ATTRIBUTE);
    let services = interface::services(&ast).expect("services");
    let bottom = services
        .iter()
        .find(|service| service.name == "Bottom")
        .expect("Bottom");
    let flattened =
        interface::all_operations_flattened(&ast, bottom.id).expect("flattened operations");
    let names: Vec<&str> = flattened.iter().map(|op| op.name.as_str()).collect();

    // The diamond: `t` (Top, through Left and Right both) appears once.
    assert_eq!(
        names.iter().filter(|name| **name == "t").count(),
        1,
        "Top's operation, reached through both Left and Right, is not doubled: {names:?}"
    );
    // The bug: `shared`'s setter (Top, not `readonly`) is not dropped
    // for looking like a second `get_attribute_shared`.
    assert!(names.contains(&"get_attribute_shared"), "{names:?}");
    assert!(
        names.contains(&"set_attribute_shared"),
        "the setter DDS-RPC 1.0 section 7.5.1.1.3 implies for `shared` is missing: {names:?}"
    );
    // Bottom's own attribute, not reached through a diamond at all,
    // same check.
    assert!(names.contains(&"get_attribute_memory"), "{names:?}");
    assert!(names.contains(&"set_attribute_memory"), "{names:?}");
    assert!(names.contains(&"b"), "{names:?}");
    assert!(names.contains(&"l"), "{names:?}");
    assert!(names.contains(&"r"), "{names:?}");
}

/// DDS-TS 1.0 §11.7: a forward declaration with no complete declaration
/// in the compilation unit is a fatal `DDS-TS-E002`. The refusal is the
/// seam's, so the C++ and Java bindings refuse it too.
#[test]
fn a_forward_declaration_without_a_definition_is_e002() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let _ = ast
        .add_forward_interface(root, "Ghost", zerodds_ast::InterfaceKind::Interface)
        .expect("the forward declaration");

    let error = interface::services(&ast).expect_err("an orphan forward declaration");
    assert_eq!(
        error,
        InterfaceError::ForwardWithoutDefinition {
            name: "Ghost".to_string()
        }
    );
    assert_eq!(
        BindingError::from(error).diagnostic_code(),
        Some("DDS-TS-E002")
    );

    assert!(interface_cpp::cpp_interface_items(&ast).is_err());
    assert!(interface_java::java_interface_items(&ast).is_err());
}

// -- C++ ---------------------------------------------------------------

/// §7.11.1.1.2 rules 1-5: two abstract classes in the module's
/// namespace, each with a public virtual destructor and the four
/// typedefs.
#[test]
fn cpp_declares_the_two_abstract_classes_of_rule_one() {
    let text = cpp(CALCULATOR);
    assert!(text.contains("namespace robot {"), "{text}");
    assert!(text.contains("class Calculator : public Base {"), "{text}");
    assert!(
        text.contains("class CalculatorAsync : public BaseAsync {"),
        "{text}"
    );
    // Rule 2.
    assert!(text.contains("virtual ~Calculator() = default;"), "{text}");
    assert!(
        text.contains("virtual ~CalculatorAsync() = default;"),
        "{text}"
    );
    // Rule 3.
    assert!(
        text.contains("using SupportType = CalculatorSupport;"),
        "{text}"
    );
    assert!(
        text.contains("using RequestType = Calculator_Request;"),
        "{text}"
    );
    assert!(
        text.contains("using ReplyType = Calculator_Reply;"),
        "{text}"
    );
    // Rules 4 and 5.
    assert!(
        text.contains("using AsyncInterfaceType = CalculatorAsync;"),
        "{text}"
    );
    assert!(text.contains("using InterfaceType = Calculator;"), "{text}");
}

/// §7.11.1.1.2 rule 6: pure virtual functions, the IDL's names and
/// argument order, no exception specification, a primitive return by
/// value (6.g) and a constructed return as a leading `cxx_return` (6.h).
#[test]
fn cpp_declares_the_synchronous_operations_of_rule_six() {
    let text = cpp(CALCULATOR);
    assert!(
        text.contains("virtual std::int32_t add(std::int32_t a, std::int32_t b) = 0;"),
        "6.a, 6.b, 6.g: {text}"
    );
    assert!(
        text.contains(
            "virtual std::int32_t divmod(std::int32_t a, std::int32_t b, std::int32_t& \
             remainder) = 0;"
        ),
        "6.e sends `out` to DDS-Cxx-PSM §7.4.5's `T&`: {text}"
    );
    assert!(
        text.contains(
            "virtual void locate(::robot::Pose& cxx_return, const std::string& room) = 0;"
        ),
        "6.h: {text}"
    );
    assert!(!text.contains("throw("), "6.c: no exception specification");
    assert!(
        text.contains("Raises, as the IDL declares it: Overflow."),
        "the clause still reaches the output: {text}"
    );
}

/// §7.11.1.1.2 rule 7: `${operation}_async`, `In`/`InOut` only, every
/// argument `const`, `future<T>` for an ordinary return (7.h) and
/// `future<${interface}_${operation}_Out>` where there are `out` or
/// `inout` parameters (7.i).
#[test]
fn cpp_declares_the_asynchronous_operations_of_rule_seven() {
    let text = cpp(CALCULATOR);
    assert!(
        text.contains(
            "virtual dds::rpc::future<std::int32_t> add_async(const std::int32_t a, const \
             std::int32_t b) = 0;"
        ),
        "7.a, 7.d, 7.h: {text}"
    );
    assert!(
        text.contains(
            "virtual dds::rpc::future<Calculator_divmod_Out> divmod_async(const std::int32_t a, \
             const std::int32_t b) = 0;"
        ),
        "7.b and 7.i: {text}"
    );
    assert!(
        text.contains("virtual dds::rpc::future<void> reset_async() = 0;"),
        "a void operation: {text}"
    );
}

/// §7.11.1.1.2 rule 6.f and 7.f/7.g: an attribute's accessors are named
/// after the attribute (DDS-Cxx-PSM §7.4.6) and not `get_`/`set_`, which
/// is the *implied IDL* of §7.5.1.1.3 and belongs to the topic types.
#[test]
fn cpp_names_an_attribute_accessor_after_the_attribute() {
    let text = cpp(CALCULATOR);
    assert!(text.contains("virtual std::int32_t total() = 0;"), "{text}");
    assert!(text.contains("virtual std::string& label() = 0;"), "{text}");
    assert!(
        text.contains("virtual const std::string& label() const = 0;"),
        "{text}"
    );
    assert!(
        text.contains("virtual void label(const std::string& label) = 0;"),
        "{text}"
    );
    assert!(
        !text.contains("virtual std::int32_t get_total()"),
        "the implied-IDL name is not the C++ accessor's: {text}"
    );
    assert!(
        text.contains("virtual dds::rpc::future<std::int32_t> total_async() = 0;"),
        "7.f: {text}"
    );
    assert!(
        text.contains("virtual dds::rpc::future<void> label_async(const std::string& label) = 0;"),
        "7.g: {text}"
    );
    // A `readonly` attribute has no setter, in either half.
    assert!(!text.contains("void total(const"), "{text}");
    assert!(!text.contains("total_async(const"), "{text}");
}

/// §7.5.1.1.4-7: the topic types, with the names
/// `crates/rpc/src/codegen.rs` derives.
#[test]
fn cpp_declares_the_topic_types_of_seven_five() {
    let text = cpp(CALCULATOR);
    for expected in [
        "struct Calculator_add_In {",
        "struct Calculator_add_Out {",
        "struct Calculator_divmod_Out {",
        // §7.5.1.1.5: one `Result` union per operation, over the `Out`
        // structure and the exceptions.
        "struct Calculator_add_Result {",
        "struct Calculator_Call {",
        // §7.5.1.1.7 rule 1: the reply union is `${I}_Return`.
        "struct Calculator_Return {",
        "struct Calculator_Request {",
        "struct Calculator_Reply {",
        "dds::rpc::RequestHeader header;",
        "dds::rpc::ReplyHeader header;",
        // §7.5.1.1.6 rule 2b / §7.5.1.1.7 rule 3b: the second member is
        // `data`.
        "Calculator_Call data;",
        "Calculator_Return data;",
        // §7.5.1.1.6 rule 3: the `default` case.
        "dds::rpc::UnknownOperation unknownOp;",
        // §7.5.1.1.5 rule 3a.
        "Calculator_add_Out result;",
        // §7.5.1.1.5 rule 5b: lower-cased exception name plus `_ex`.
        "Overflow overflow_ex;",
    ] {
        assert!(text.contains(expected), "{expected} missing from:\n{text}");
    }
    // §7.5.1.1.6 rule 4: the case-label constants stand beside the
    // unions, so a reader sees which value selects which case.
    assert!(
        text.contains("inline constexpr std::int32_t Calculator_add_Hash = "),
        "{text}"
    );
    assert!(
        text.contains("inline constexpr std::int32_t Overflow_Ex_Hash = "),
        "{text}"
    );
    // §7.5.1.1.4 rule 3c and §7.5.1.1.5 rule 3c: `reset` has neither
    // parameters nor a return.
    assert!(text.contains("dds::rpc::UnusedMember dummy;"), "{text}");
    // §7.5.1.1.5 rule 3b: the return value is the *last* member.
    let divmod_out = text
        .split("struct Calculator_divmod_Out {")
        .nth(1)
        .expect("the Out structure");
    let remainder = divmod_out.find("remainder").expect("the out parameter");
    let returns = divmod_out.find("return_").expect("the return value");
    assert!(
        remainder < returns,
        "the return value comes last:\n{divmod_out}"
    );
    // §7.5.1.1.7 rule 5: every operation has a case in the Return union,
    // a `void` one included, over an `_Out` structure of no members.
    assert!(text.contains("Calculator_reset_In reset;"), "{text}");
    assert!(text.contains("struct Calculator_reset_Out {"), "{text}");
    // §7.5.1.1.3's implied operations reach the topic types under the
    // implied names.
    assert!(
        text.contains("struct Calculator_get_attribute_total_In {"),
        "{text}"
    );
    assert!(
        text.contains("struct Calculator_set_attribute_label_In {"),
        "{text}"
    );
}

/// §7.5.1.2.4 and §7.5.1.2.5: the Enhanced Service Mapping names its
/// topic types `${I}_Request` and `${I}_Reply`, one pair per interface
/// (§7.4.3), not one pair per operation. The per-operation
/// `${I}_${op}_Request` types the generator once declared are not in the
/// spec.
#[test]
fn cpp_declares_no_per_operation_request_types() {
    let text = cpp(CALCULATOR);
    assert!(!text.contains("struct Calculator_add_Request"), "{text}");
    assert!(!text.contains("struct Calculator_add_Reply"), "{text}");
    assert!(text.contains("struct Calculator_Request {"), "{text}");
}

/// §7.11.1.1.1: the typed per-service `Requester` and `Replier` over the
/// generic templates.
#[test]
fn cpp_binds_the_requester_and_replier_of_seven_eleven_one_one_one() {
    let text = cpp(CALCULATOR);
    assert!(
        text.contains(
            "using CalculatorRequester = dds::rpc::core::Requester<Calculator_Request, \
             Calculator_Reply>;"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "using CalculatorReplier = dds::rpc::core::Replier<Calculator_Request, Calculator_Reply>;"
        ),
        "{text}"
    );
}

/// §7.11.1.5.1 and §7.11.1.5.4: the function-call `Service` over an
/// implementation, and the `Client` inheriting from `${interface}`,
/// `${interface}Async` and `ClientEndpoint` (public virtual).
#[test]
fn cpp_binds_the_function_call_entities_of_seven_eleven_one_five() {
    let text = cpp(CALCULATOR);
    assert!(
        text.contains(
            "class CalculatorClient : public Calculator, public CalculatorAsync, public virtual \
             dds::rpc::Client {"
        ),
        "{text}"
    );
    // §7.11.1.5.1: every Service inherits from ServiceEndpoint — through
    // `dds::rpc::Service`, which a `Server` takes (§7.11.1.5.3).
    assert!(
        text.contains("class CalculatorService : public dds::rpc::Service {"),
        "{text}"
    );
    assert!(
        text.contains(
            "CalculatorService(Calculator& implementation, dds::rpc::RPCRuntime& runtime)"
        ),
        "a Service accepts a reference to the implementation: {text}"
    );
    assert!(
        text.contains(
            "CalculatorService(Calculator& implementation, const dds::rpc::ServiceParams& params)"
        ),
        "{text}"
    );
}

/// §7.9.1, §7.11.1.5.4: the client's operations are bodies that call
/// through the requester: the `Call` union is packed, `call` sends it and
/// the `Result` union of the reply is unpacked.
#[test]
fn cpp_client_operations_call_through_the_requester() {
    let text = cpp(CALCULATOR);
    assert!(
        text.contains("requester_.call(zd_request).data()"),
        "{text}"
    );
    assert!(
        text.contains("zd_request.data._d = Calculator_add_Hash;"),
        "the case label is the hash of the operation: {text}"
    );
    assert!(
        text.contains(
            "static const Calculator_add_Out& result_add(const Calculator_Reply& zd_reply)"
        ),
        "{text}"
    );
    assert!(
        text.contains("throw dds::rpc::RemoteUnknownOperationError"),
        "a reply of another operation is a remote exception: {text}"
    );
    // §7.11.1.1.2 rule 7: the asynchronous operations hand the reply to a
    // future through `send_request_mapped`.
    assert!(
        text.contains("requester_.send_request_mapped(zd_request, &unpack_add)"),
        "{text}"
    );
}

/// §7.5.1.1.5: an exception an operation raises is thrown at the caller by
/// the client and caught around the implementation call by the service.
#[test]
fn cpp_client_throws_and_service_catches_a_raised_exception() {
    let text = cpp("exception TooFast { long limit; };
         interface Drive { long go(in long speed) raises (TooFast); };");
    assert!(text.contains("throw zd_result.toofast_ex;"), "{text}");
    assert!(text.contains("catch (const TooFast& zd_error)"), "{text}");
    assert!(
        text.contains("zd_reply.data.go._d = TooFast_Ex_Hash;"),
        "{text}"
    );
}

/// §7.5.1.1.6: the service answers an operation it has no case for with a
/// remote exception code, and turns what the implementation throws into
/// one (§7.5.2).
#[test]
fn cpp_service_translates_what_the_implementation_throws() {
    let text = cpp(CALCULATOR);
    assert!(
        // The caught exception is kept in `zd_error`, which the service
        // rethrows when it has no reply to send, so the code is read from it.
        text.contains("dds::rpc::detail::remote_exception_code_of(zd_error)"),
        "{text}"
    );
    assert!(
        text.contains("throw dds::rpc::RemoteUnsupportedError(\"the interface has no operation"),
        "{text}"
    );
}

/// §7.5.1.1: the codec of the two topic types, which the requester and the
/// replier ask `dds_type_traits` for.
#[test]
fn cpp_writes_the_codec_of_the_topic_types() {
    let text = cpp(CALCULATOR);
    assert!(
        text.contains("struct dds_type_traits<::robot::Calculator_Request>"),
        "{text}"
    );
    assert!(
        text.contains("return \"robot::Calculator_Request\";"),
        "the DDS type name is the scoped name: {text}"
    );
    assert!(
        text.contains("static dds::rpc::Octets serialize_at"),
        "{text}"
    );
    assert!(text.contains("#include <dds/rpc.hpp>"), "{text}");
}

/// §7.5.1.1.8: a client of a derived interface reaches the operations of a
/// base through the base's own client, on the base's own topics, and a
/// service answers on both.
#[test]
fn cpp_derived_client_and_service_use_the_clients_and_services_of_the_bases() {
    let text = cpp(CALCULATOR);
    assert!(text.contains("::robot::BaseClient zd_base_0_;"), "{text}");
    assert!(text.contains("::robot::BaseService zd_base_0_;"), "{text}");
    assert!(
        text.contains("zd_answered += zd_base_0_.dispatch();"),
        "{text}"
    );
}

/// §7.11.1.1.2 rule 8: an identifier that is a C++ reserved word is
/// prefixed with `cxx_`.
#[test]
fn cpp_prefixes_a_reserved_word_with_cxx() {
    let text = cpp("interface Ops { void delete(in long template); };");
    assert!(
        text.contains("virtual void cxx_delete(std::int32_t cxx_template) = 0;"),
        "{text}"
    );
    assert!(text.contains("struct Ops_cxx_delete_In {"), "{text}");
}

/// The complete C++ declarations of one IDL source: the types the adapter
/// declares and the RPC binding of the interfaces.
fn cpp_complete(idl: &str) -> String {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    zerodds_lang_rules::generate::declarations(&model(idl), zerodds_lang_cst::Language::Cpp)
        .unwrap_or_else(|error| panic!("{error:?}"))
}

/// §7.11.1.1.2 rule 8 for an exception: the declaration and every reference
/// spell the reserved word with the `cxx_` prefix, and nothing is declared
/// under the underscore spelling.
#[test]
fn cpp_names_a_reserved_exception_with_the_cxx_prefix() {
    let text = cpp_complete(
        "exception template { long code; };
         interface Ops { void go() raises (template); };",
    );
    assert!(text.contains("struct cxx_template"), "{text}");
    assert!(!text.contains("struct template_"), "{text}");
    // The union member is named after the exception, lower case with `_ex`
    // (§7.5.1.1.5 rule 5b); `template_ex` is not a reserved word, so rule 8
    // does not prefix it. Only the type it holds is spelled with the prefix.
    assert!(text.contains("cxx_template template_ex;"), "{text}");
    assert!(!text.contains("cxx_template_ex"), "{text}");
}

/// The module is a reserved word too (DDS-Cxx-PSM Table 7.1 reserves the
/// C++ keywords): the namespace, the type declared in it and every
/// reference into it are `cxx_` prefixed, the topic-type traits included.
#[test]
fn cpp_names_a_reserved_module_and_type_with_the_cxx_prefix() {
    let text = cpp_complete(
        "module class { struct delete { long x; };
                       interface Ops { delete get(); }; };",
    );
    assert!(text.contains("namespace cxx_class"), "{text}");
    assert!(text.contains("struct cxx_delete"), "{text}");
    assert!(!text.contains("namespace class_"), "{text}");
    assert!(!text.contains("struct delete_"), "{text}");
    assert!(!text.contains("::class_::"), "{text}");
    assert!(text.contains("::cxx_class::cxx_delete"), "{text}");
    assert!(
        text.contains("dds_type_traits<::cxx_class::Ops_Request>"),
        "{text}"
    );
}

/// DDS-RPC 1.0 §7.3.1.1 production (7): `<interface_header>` admits the
/// plain `interface` form only, so a `local interface` is refused where
/// the seam reads it, and no binding writes a class for it.
#[test]
fn a_local_interface_is_refused_by_the_seam() {
    let error = refused("local interface Cache { long size(); };");
    assert!(
        matches!(
            error,
            InterfaceError::OutsideServiceGrammar {
                construct: "an interface declaration of a form other than `interface`",
                ..
            }
        ),
        "{error:?}"
    );
    let ast = model_lenient("local interface Cache { long size(); };");
    assert!(interface_cpp::cpp_interface_items(&ast).is_err());
    assert!(interface_java::java_interface_items(&ast).is_err());
}

// -- Java --------------------------------------------------------------

/// §7.11.2.3: the same entities as C++, in Java's spelling — two
/// interfaces, the endpoints, and the function-call client and service.
#[test]
fn java_declares_the_same_entities_as_cpp() {
    let text = java(CALCULATOR);
    assert!(text.contains("public final class ZdServices {"), "{text}");
    assert!(text.contains("class robot {"), "{text}");
    assert!(
        text.contains("public interface Calculator extends Base {"),
        "{text}"
    );
    assert!(
        text.contains("public interface CalculatorAsync extends BaseAsync {"),
        "{text}"
    );
    assert!(
        text.contains(
            "public static class CalculatorRequester extends \
             org.zerodds.ddsrpc.RequesterImpl<Calculator_Request, Calculator_Reply> {"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "public interface CalculatorClient extends Calculator, CalculatorAsync, \
             org.omg.dds.rpc.ClientEndpoint {"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "public interface CalculatorService extends org.omg.dds.rpc.ServiceEndpoint {"
        ),
        "{text}"
    );
}

/// §7.11.2.2.4: an asynchronous operation returns
/// `org.omg.dds.rpc.Future<T>`, with the wrapper class where the type
/// argument cannot be a primitive, and the `_Out` type where the call
/// has more than one result.
#[test]
fn java_returns_a_future_of_the_boxed_result() {
    let text = java(CALCULATOR);
    assert!(
        text.contains("org.omg.dds.rpc.Future<Integer> add_async(int a, int b);"),
        "{text}"
    );
    assert!(
        text.contains("org.omg.dds.rpc.Future<Calculator_divmod_Out> divmod_async(int a, int b);"),
        "{text}"
    );
    assert!(
        text.contains("org.omg.dds.rpc.Future<Void> reset_async();"),
        "{text}"
    );
}

/// Java has no `T&`, so the multiple return of an `out`/`inout` call
/// travels in the `${interface}_${operation}_Out` type §7.11.1.1.2 rule
/// 7.i names for the same call shape.
#[test]
fn java_returns_the_out_type_where_cpp_takes_a_reference() {
    let text = java(CALCULATOR);
    assert!(text.contains("int add(int a, int b);"), "{text}");
    assert!(
        text.contains("Calculator_divmod_Out divmod(int a, int b);"),
        "{text}"
    );
}

/// §7.11.1.1.2 rule 6.c, which §7.11.2 does not override: no exception
/// specification. The clause still reaches the generated file, as
/// documentation.
#[test]
fn java_declares_no_throws_and_still_names_the_raises() {
    let text = java(CALCULATOR);
    assert!(!text.contains("throws"), "{text}");
    assert!(
        text.contains("Raises, as the IDL declares it: Overflow."),
        "{text}"
    );
}

/// §7.2.2.3, §7.9.1, §7.11.1.5 and §7.11.2.3: the client and the service are
/// not only declared. The client packs a request and unpacks a reply,
/// throws what the reply carries and forwards the operations of its base;
/// the service unpacks, calls the implementation and packs, and answers an
/// operation it does not know with REMOTE_EX_UNSUPPORTED.
#[test]
fn java_function_call_classes_have_bodies() {
    let text = java(CALCULATOR);
    for fragment in [
        // The three pieces of one interface.
        "class CalculatorCodec",
        "class CalculatorClientImpl extends org.omg.dds.rpc.FunctionCallClient<Calculator_Request, Calculator_Reply>",
        "class CalculatorServiceImpl extends org.omg.dds.rpc.FunctionCallService<Calculator_Request, Calculator_Reply>",
        // The client: pack, call, unpack, forward.
        "reset_request()",
        "this.base0.ping()",
        "new org.omg.dds.rpc.UserExceptionRaised(result.overflow_ex)",
        "org.omg.dds.rpc.Futures.map(callAsync(add_request(a, b))",
        // The service: call the implementation, unknown operation.
        "org.omg.dds.rpc.Outcome.of(() -> implementation.add(in.a, in.b))",
        "REMOTE_EX_UNSUPPORTED",
        // §7.5.1.1.5: the `default` case of the Result union.
        "unknownEx",
        // The settings reach the base interface's endpoints.
        "sameSettingsAs(params)",
        // §7.5.1.1.6: the union starts on its own alignment.
        "org.omg.dds.rpc.WireEncoding",
    ] {
        assert!(text.contains(fragment), "missing `{fragment}` in {text}");
    }
}

/// §7.4.1 and §7.4.2.3: the Java factories hand the runtime the
/// `<interface_name>` and the annotated service name, so a service name
/// set at run time composes `<interface_name>_<service_name>_Request` and
/// not the request-reply style's `<service_name>_Request`.
#[test]
fn java_support_names_the_interface_for_a_run_time_service_name() {
    let text = java(CALCULATOR);
    assert!(
        text.contains("public static final String INTERFACE_NAME = \"robot_Calculator\";"),
        "{text}"
    );
    for fragment in [
        ".interfaceName(INTERFACE_NAME)",
        ".annotatedServiceName(SERVICE_NAME)",
    ] {
        assert!(text.contains(fragment), "missing `{fragment}` in {text}");
    }
}

/// §7.4.1.1's two topic names, as the constants a Java service reads
/// them from.
#[test]
fn java_carries_the_topic_names_as_constants() {
    let text = java(CALCULATOR);
    // §7.4.1: `<interface_name>_<service_name>_Request`, with the
    // interface name fully qualified and `<service_name>` at its
    // `"Service"` default.
    assert!(
        text.contains(
            "public static final String REQUEST_TOPIC_NAME = \
             \"robot_Calculator_Service_Request\";"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "public static final String REPLY_TOPIC_NAME = \"robot_Calculator_Service_Reply\";"
        ),
        "{text}"
    );
    // §7.5.1.1.6-7: the type names, which are the other string.
    assert!(
        text.contains(
            "public static final String REQUEST_TYPE_NAME = \"robot::Calculator_Request\";"
        ),
        "{text}"
    );
    // §7.4.3: `Calculator : Base`, so the parent contributes an alias.
    assert!(
        text.contains(
            "public static final String REQUEST_TOPIC_ALIAS_0 = \"robot_Base_Service_Request\";"
        ),
        "{text}"
    );
}

/// §7.4.1, §7.4.2.2 and §7.4.3 through the C++ column: the two names,
/// the `<service_name>` segment, the aliases, and the annotation that
/// replaces the whole name.
#[test]
fn cpp_carries_the_topic_names_of_seven_four() {
    let text = cpp(CALCULATOR);
    assert!(
        text.contains(
            "static constexpr const char* request_topic_name() noexcept { return \
             \"robot_Calculator_Service_Request\"; }"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "static constexpr const char* service_name() noexcept { return \"Service\"; }"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "static constexpr const char* request_topic_alias_0() noexcept { return \
             \"robot_Base_Service_Request\"; }"
        ),
        "§7.4.3's alias from the parent interface: {text}"
    );

    // §7.4.2.2: the annotation gives a whole topic name, and §7.4.3
    // gives it no alias — there is no interface segment in it to vary.
    let annotated = cpp(r#"module robot {
             @service(name = "Arithmetic")
             @DDSRequestTopic(name = "MyReq")
             interface Solo { long add(in long a); };
           };"#);
    assert!(
        annotated.contains(
            "static constexpr const char* request_topic_name() noexcept { return \"MyReq\"; }"
        ),
        "{annotated}"
    );
    assert!(
        annotated.contains(
            "static constexpr const char* reply_topic_name() noexcept { return \
             \"robot_Solo_Arithmetic_Reply\"; }"
        ),
        "the reply has no annotation, so it is synthesized: {annotated}"
    );
    assert!(
        !annotated.contains("request_topic_alias_0"),
        "a whole-name annotation has no alias: {annotated}"
    );
}

// -- TypeScript --------------------------------------------------------

/// DDS-TS 1.0 §11.1: a client interface, a handler interface and a
/// service descriptor. No class.
#[test]
fn typescript_maps_an_interface_to_a_client_a_handler_and_a_descriptor() {
    let text = typescript(CALCULATOR);
    assert!(text.contains("export namespace robot {"), "{text}");
    assert!(
        text.contains("export interface CalculatorClient extends BaseClient {"),
        "§11.6: {text}"
    );
    assert!(
        text.contains("export interface CalculatorHandler extends BaseHandler {"),
        "§11.6: {text}"
    );
    assert!(
        text.contains(
            "export const CalculatorService: ServiceDescriptor<CalculatorClient, \
             CalculatorHandler> ="
        ),
        "§11.5: {text}"
    );
    assert!(!text.contains("class"), "§11.1 emits no class: {text}");
    assert!(!text.contains("async "), "§11.2 emits no `async`: {text}");
}

/// §11.2: `Promise<T>`, the `in`/`inout` parameters only, and the
/// aggregated resolved value where the operation declares `out` or
/// `inout`.
#[test]
fn typescript_aggregates_out_parameters_into_the_resolved_value() {
    let text = typescript(CALCULATOR);
    assert!(
        text.contains("add(a: number, b: number): Promise<number>;"),
        "{text}"
    );
    assert!(
        text.contains(
            "divmod(a: number, b: number): Promise<{ result: number; remainder: number }>;"
        ),
        "{text}"
    );
    assert!(text.contains("reset(): Promise<void>;"), "{text}");
}

/// §11.3: `get_<name>` always, `set_<name>` unless the attribute is
/// `readonly`.
#[test]
fn typescript_maps_an_attribute_to_a_getter_and_a_setter() {
    let text = typescript(CALCULATOR);
    assert!(text.contains("get_total(): Promise<number>;"), "{text}");
    assert!(text.contains("get_label(): Promise<string>;"), "{text}");
    assert!(
        text.contains("set_label(value: string): Promise<void>;"),
        "{text}"
    );
    assert!(!text.contains("set_total("), "`total` is readonly: {text}");
}

/// §11.5: the descriptor's four fields, with `inherits` in declaration
/// order and nothing flattened into it (§11.6).
#[test]
fn typescript_writes_the_service_descriptor_of_eleven_five() {
    let text = typescript(CALCULATOR);
    assert!(text.contains("name: \"robot::Calculator\""), "{text}");
    assert!(text.contains("inherits: [BaseService]"), "{text}");
    assert!(text.contains("oneway: false"), "the `oneway` flag: {text}");
    assert!(text.contains("raises: [OverflowType]"), "§11.4: {text}");
    assert!(
        text.contains("mode: \"out\""),
        "every parameter, not only the inputs: {text}"
    );
    assert!(
        text.contains("returnType: { kind: \"void\" }"),
        "the void marker: {text}"
    );
    assert!(
        text.contains("kind: \"ref\", name: \"robot::Pose\""),
        "a constructed return is a `ref` to its qualified IDL name: {text}"
    );
    assert!(
        text.contains("kind: \"primitive\", name: \"int32\""),
        "{text}"
    );
    assert!(text.contains("kind: \"string\", wide: false"), "{text}");
}

/// The `bound` property is absent for an unbounded collection, not
/// `undefined`: the runtime is compiled with
/// `exactOptionalPropertyTypes`.
#[test]
fn typescript_omits_an_absent_bound() {
    let text = typescript(
        "interface Store {
           void put(in sequence<long, 4> bounded, in sequence<long> unbounded);
         };",
    );
    assert!(
        text.contains(
            "kind: \"sequence\", element: { kind: \"primitive\", name: \"int32\" }, bound: 4"
        ),
        "{text}"
    );
    assert!(!text.contains("bound: undefined"), "{text}");
}

// -- the driver --------------------------------------------------------

/// `generate::declarations` is the one door into the materialisers, and
/// the RPC binding goes through it: a binding reachable only from a test
/// is a binding no generated file ever gets.
#[test]
fn the_three_bindings_reach_generate_declarations() {
    let ast = model(CALCULATOR);
    for (language, expected) in [
        (Language::Cpp, "class CalculatorAsync"),
        (Language::Java, "public interface CalculatorAsync"),
        (Language::TypeScript, "export interface CalculatorHandler"),
    ] {
        let text = zerodds_lang_rules::generate::declarations(&ast, language)
            .unwrap_or_else(|error| panic!("{language:?}: {error}"));
        assert!(
            text.contains(expected),
            "{language:?}: `{expected}` missing from:\n{text}"
        );
    }
}

/// A model with no `interface` in it produces no binding at all, in any
/// of the three columns — and still produces its data types.
#[test]
fn a_model_without_an_interface_produces_no_binding() {
    let ast = model("struct Pose { long x; };");
    assert!(interface::services(&ast).expect("services").is_empty());
    assert!(
        interface_cpp::cpp_interface_items(&ast)
            .expect("cpp")
            .is_empty()
    );
    assert!(
        interface_java::java_interface_items(&ast)
            .expect("java")
            .is_empty()
    );
    let graph = ModelGraph::of(&ast);
    let applied = apply(&graph, &rule::catalogue(Language::TypeScript)).expect("apply");
    assert!(
        interface_typescript::ts_interface_items(&ast, &applied)
            .expect("typescript")
            .is_empty()
    );
}

const NESTED_STORE: &str = "module nested {
    @appendable struct Appended { long a; double b; };
    @mutable struct Mutated { long m; @id(7) long n; };
    @DDSService interface Store { long put(in Appended a, in Mutated m); };
};";

/// A nested `@appendable` or `@mutable` member of a topic type is written
/// in the form of the selected encoding (XTypes 1.3 §7.4.1.1, §7.4.1.2):
/// the C++ binding has the member codecs under both, and the union
/// codecs under classic CDR in their own namespace.
#[test]
fn cpp_writes_a_nested_member_in_the_form_of_the_selected_encoding() {
    let text = cpp(NESTED_STORE);
    let classic_at = text.find("namespace zd_xcdr1").expect("classic CDR codecs");
    let classic = &text[classic_at..];
    assert!(classic.contains("no_dheader"), "{classic}");
    assert!(classic.contains("write_pid"), "{classic}");
    assert!(text.contains("write_dheader"), "{text}");
    assert!(text.contains("write_emheader"), "{text}");
    assert!(
        text.contains("zd_xcdr1::zd_enc_rpc_nested_Store_Call"),
        "{text}"
    );
    assert!(
        text.contains("encoding == ::dds::rpc::WireEncoding::XCDR1")
            || text.contains("encoding == dds::rpc::WireEncoding::XCDR1"),
        "{text}"
    );
}

/// The Java binding calls `ZdCodec.Xcdr1` for the members of a topic type
/// when the writer or the reader was made for classic CDR.
#[test]
fn java_writes_a_nested_member_in_the_form_of_the_selected_encoding() {
    let text = java(NESTED_STORE);
    assert!(
        text.contains("ZdCodec.Xcdr1.zd_enc_nested_Appended"),
        "{text}"
    );
    assert!(text.contains("ZdCodec.zd_enc_nested_Appended"), "{text}");
    assert!(
        text.contains("org.omg.dds.rpc.WireEncoding.XCDR1.maxAlign()"),
        "{text}"
    );
}

// -- §7.3.1.1: what the service grammar removed ------------------------

/// Asserts that `idl` is refused with `OutsideServiceGrammar` naming
/// `construct`.
fn assert_outside_grammar(idl: &str, construct: &str) {
    match refused(idl) {
        InterfaceError::OutsideServiceGrammar { construct: got, .. } => {
            assert_eq!(got, construct, "{idl}");
        }
        other => panic!("{idl}: {other:?}"),
    }
}

/// §7.3.1.1 production (87) removed `oneway`; §7.5.1.1.7 rule 5 gives
/// every operation a Return-union case.
#[test]
fn the_seam_refuses_a_oneway_operation() {
    assert_outside_grammar(
        "interface I { oneway void log(in string msg); };",
        "a `oneway` operation",
    );
}

/// §7.3.2 keeps `@oneway` for Java source only.
#[test]
fn the_seam_refuses_the_oneway_annotation() {
    assert_outside_grammar(
        "interface I { @oneway void ping(); };",
        "a `oneway` operation",
    );
}

/// Production (87) removed `context` expressions.
#[test]
fn the_seam_refuses_a_context_clause() {
    assert_outside_grammar(
        "interface I { void ping() context (\"x\"); };",
        "a `context` clause",
    );
}

/// Production (9): a type declaration in the interface body.
#[test]
fn the_seam_refuses_a_type_in_an_interface_body() {
    assert_outside_grammar(
        "interface I { struct S { long a; }; void ping(); };",
        "a type declaration in an interface body",
    );
}

/// Production (9): a constant declaration in the interface body.
#[test]
fn the_seam_refuses_a_constant_in_an_interface_body() {
    assert_outside_grammar(
        "interface I { const long N = 1; void ping(); };",
        "a constant declaration in an interface body",
    );
}

/// Production (9): an exception declaration in the interface body.
#[test]
fn the_seam_refuses_an_exception_in_an_interface_body() {
    assert_outside_grammar(
        "interface I { exception E { long c; }; void ping(); };",
        "an exception declaration in an interface body",
    );
}

/// Production (1): the `import` directive.
#[test]
fn the_seam_refuses_an_import_directive() {
    assert_outside_grammar(
        "import X; interface I { void ping(); };",
        "an `import` directive",
    );
}

/// Production (2): `typeid` and `typeprefix`.
#[test]
fn the_seam_refuses_typeid_and_typeprefix_directives() {
    assert_outside_grammar(
        "typeid I \"IDL:I:1.0\"; interface I { void ping(); };",
        "a `typeid`/`typeprefix` directive",
    );
    assert_outside_grammar(
        "typeprefix I \"IDL:example/\"; interface I { void ping(); };",
        "a `typeid`/`typeprefix` directive",
    );
}

/// Production (7): `abstract interface`.
#[test]
fn the_seam_refuses_an_abstract_interface() {
    assert_outside_grammar(
        "abstract interface I { void ping(); };",
        "an interface declaration of a form other than `interface`",
    );
}

/// Production (2) and (7): `component`.
#[test]
fn the_seam_refuses_a_component() {
    assert_outside_grammar(
        "component C { };",
        "an interface declaration of a form other than `interface`",
    );
}

/// §7.5.1.1.3 rule 1: an attribute `x` implies `get_attribute_x`, and a
/// user-defined operation of that name is illegal beside it.
#[test]
fn the_seam_refuses_an_operation_named_like_an_implied_getter() {
    let error = refused("interface I { attribute long x; long get_attribute_x(); };");
    assert_eq!(
        error,
        InterfaceError::DuplicateOperation {
            interface: "I".to_string(),
            name: "get_attribute_x".to_string(),
        }
    );
}

/// Rule 1, setter half. A `readonly` attribute implies no setter, so the
/// same name is legal there.
#[test]
fn the_seam_refuses_an_operation_named_like_an_implied_setter() {
    let error = refused("interface I { attribute long x; void set_attribute_x(in long v); };");
    assert_eq!(
        error,
        InterfaceError::DuplicateOperation {
            interface: "I".to_string(),
            name: "set_attribute_x".to_string(),
        }
    );
    let ast = model("interface I { readonly attribute long x; void set_attribute_x(in long v); };");
    interface::services(&ast).expect("a readonly attribute implies no setter");
}

/// Two declared operations of one name. The frontend refuses to build
/// such a model from IDL, so it is built through the model API.
#[test]
fn the_seam_refuses_two_operations_of_one_name() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let iface = ast
        .add_interface(root, "I", zerodds_ast::InterfaceKind::Interface, Vec::new())
        .expect("an interface");
    ast.add_operation(iface, "f", None, false)
        .expect("an operation");
    ast.add_operation(iface, "f", None, false)
        .expect("a second operation");
    let error = interface::services(&ast).expect_err("a duplicate operation");
    assert_eq!(
        error,
        InterfaceError::DuplicateOperation {
            interface: "I".to_string(),
            name: "f".to_string(),
        }
    );
}

/// Two parameters of one name, built through the model API as above.
#[test]
fn the_seam_refuses_two_parameters_of_one_name() {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let iface = ast
        .add_interface(root, "I", zerodds_ast::InterfaceKind::Interface, Vec::new())
        .expect("an interface");
    let long = ast
        .add_type(
            root,
            "Long",
            zerodds_ast::TypeKind::Primitive(zerodds_ast::PrimitiveKind::Int32),
        )
        .expect("a type");
    let op = ast
        .add_operation(iface, "f", None, false)
        .expect("an operation");
    for _ in 0..2 {
        ast.add_param(
            op,
            "a",
            zerodds_ast::ParamDirection::In,
            zerodds_ast_core::Ref::Resolved(long),
        )
        .expect("a parameter");
    }
    let error = interface::services(&ast).expect_err("a duplicate parameter");
    assert!(
        matches!(error, InterfaceError::DuplicateParameter { ref name, .. } if name == "a"),
        "{error:?}"
    );
}

// -- §7.5.1.1.3 rules 3 to 5, §7.5.1.1.5 rule 3, placement ------------

/// The body of the C++ `struct`/`class` called `name`, up to the next
/// `struct `.
fn struct_body<'a>(text: &'a str, name: &str) -> &'a str {
    let header = format!("struct {name} {{");
    let after = text
        .split(header.as_str())
        .nth(1)
        .unwrap_or_else(|| panic!("no `{header}` in:\n{text}"));
    after.split("struct ").next().unwrap_or(after)
}

/// §7.5.1.1.3 rule 3: the implied setter takes one argument of the
/// attribute's type, named like the attribute.
#[test]
fn the_implied_setter_takes_one_argument_named_like_the_attribute() {
    let ast = model(CALCULATOR);
    let services = interface::services(&ast).expect("services");
    let calculator = services
        .iter()
        .find(|service| service.name == "Calculator")
        .expect("Calculator");
    let all = calculator.all_operations();
    let setter = all
        .iter()
        .find(|operation| operation.name == "set_attribute_label")
        .expect("the setter");
    assert_eq!(setter.params.len(), 1);
    assert_eq!(setter.params[0].name, "label");
    assert_eq!(setter.params[0].direction, zerodds_ast::ParamDirection::In);
    let label = calculator
        .attributes
        .iter()
        .find(|attribute| attribute.name == "label")
        .expect("the attribute");
    assert_eq!(setter.params[0].type_, label.type_);
    // The topic type carries the member under that name (rule 3 of
    // §7.5.1.1.4 takes the formal parameter names).
    let text = cpp(CALCULATOR);
    assert!(
        struct_body(&text, "Calculator_set_attribute_label_In").contains(" label;"),
        "{text}"
    );
}

const GUARDED: &str = "
    module faults {
      exception NotReadable { long why; };
      exception NotWritable { long why; };
    };
    interface Guarded {
      attribute long level getraises (faults::NotReadable) setraises (faults::NotWritable);
    };
";

/// §7.5.1.1.3 rules 4 and 5: `getraises` is the `raises` of the implied
/// getter and `setraises` that of the implied setter, and each
/// operation's Result union has a case for its exception.
#[test]
fn getraises_and_setraises_become_the_raises_of_the_implied_operations() {
    let ast = model(GUARDED);
    let services = interface::services(&ast).expect("services");
    let all = services[0].all_operations();
    let raised = |name: &str| -> Vec<String> {
        all.iter()
            .find(|operation| operation.name == name)
            .expect("an implied operation")
            .raises
            .iter()
            .map(|raise| raise.name.clone())
            .collect()
    };
    assert_eq!(raised("get_attribute_level"), ["NotReadable"]);
    assert_eq!(raised("set_attribute_level"), ["NotWritable"]);

    let text = cpp(GUARDED);
    let getter = struct_body(&text, "Guarded_get_attribute_level_Result");
    assert!(getter.contains("notreadable_ex;"), "{getter}");
    assert!(!getter.contains("notwritable_ex;"), "{getter}");
    let setter = struct_body(&text, "Guarded_set_attribute_level_Result");
    assert!(setter.contains("notwritable_ex;"), "{setter}");
    assert!(!setter.contains("notreadable_ex;"), "{setter}");
}

/// §7.5.1.1.5 rule 3a.i: the `inout` and `out` parameters come in the
/// order of the formal parameters, and rule 3b puts the return value
/// last. The `in` parameter is no member of the Out structure.
#[test]
fn the_out_structure_keeps_the_order_of_the_parameters() {
    let idl = "interface Mixer {
        void mix(in long a, inout long b, out double c);
        long mix2(out double c, in long a, inout long b);
    };";
    let text = cpp(idl);
    let mix = struct_body(&text, "Mixer_mix_Out");
    let (b, c) = (mix.find(" b;").expect("b"), mix.find(" c;").expect("c"));
    assert!(b < c, "{mix}");
    assert!(
        !mix.contains(" a;"),
        "an `in` parameter is no Out member: {mix}"
    );
    let mix2 = struct_body(&text, "Mixer_mix2_Out");
    let positions: Vec<usize> = [" c;", " b;", " return_;"]
        .iter()
        .map(|needle| {
            mix2.find(needle)
                .unwrap_or_else(|| panic!("{needle} in {mix2}"))
        })
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{mix2}");

    let java_text = java(idl);
    let java_mix2 = java_text
        .split("class Mixer_mix2_Out")
        .nth(1)
        .expect("the Java Out class");
    let java_mix2 = java_mix2.split("class ").next().unwrap_or(java_mix2);
    let positions: Vec<usize> = [" c;", " b;", " return_;"]
        .iter()
        .map(|needle| {
            java_mix2
                .find(needle)
                .unwrap_or_else(|| panic!("{needle} in {java_mix2}"))
        })
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{java_mix2}");
}

/// §7.5.1.1.5 rule 3b: an `out` parameter named `return_` moves the
/// regular return value to `return_1`.
#[test]
fn a_colliding_return_member_is_numbered_in_cpp_and_java() {
    let idl = "interface Odd { long f(out long return_); };";
    let text = cpp(idl);
    let out = struct_body(&text, "Odd_f_Out");
    let first = out.find(" return_;").expect("the out parameter");
    let second = out.find(" return_1;").expect("the renumbered return value");
    assert!(first < second, "{out}");

    let java_text = java(idl);
    let class = java_text
        .split("class Odd_f_Out")
        .nth(1)
        .expect("the Java Out class");
    let class = class.split("class ").next().unwrap_or(class);
    let first = class.find(" return_;").expect("the out parameter");
    let second = class
        .find(" return_1;")
        .expect("the renumbered return value");
    assert!(first < second, "{class}");
}

/// The same collision in the Rust generator, which has its own copy of
/// the rule.
#[test]
fn a_colliding_return_member_is_numbered_in_rust() {
    let ast = model("interface Odd { long f(out long return_); };");
    let items = zerodds_lang_rules::emit::interface_rust::rust_interface_pairs(&ast)
        .unwrap_or_else(|error| panic!("{error}"));
    let fields: Vec<String> = items
        .iter()
        .find_map(|(_, item)| match item {
            zerodds_lang_cst::rust::Item::Struct(declared)
                if declared.name.as_str() == "Odd_f_Out" =>
            {
                match &declared.body {
                    zerodds_lang_cst::rust::StructBody::Named(fields) => Some(
                        fields
                            .iter()
                            .map(|field| field.name.as_str().to_string())
                            .collect(),
                    ),
                    _ => None,
                }
            }
            _ => None,
        })
        .expect("Odd_f_Out");
    assert_eq!(fields, ["return_", "return_1"]);
}

/// Where the declarations of `decls` stand: `(namespace path, name)`
/// for every class and variable.
fn cpp_placement(decls: &[cpp::Decl], path: &str, out: &mut Vec<(String, String)>) {
    for decl in decls {
        match decl {
            cpp::Decl::Namespace(namespace) => {
                let inner = if path.is_empty() {
                    namespace.name.as_str().to_string()
                } else {
                    format!("{path}::{}", namespace.name.as_str())
                };
                cpp_placement(&namespace.decls, &inner, out);
            }
            cpp::Decl::Class(class) => {
                out.push((path.to_string(), class.name.as_str().to_string()));
            }
            cpp::Decl::Variable(variable) => {
                out.push((path.to_string(), variable.name.as_str().to_string()));
            }
            _ => {}
        }
    }
}

/// §7.5.1.1.4 rule 2, §7.5.1.1.5 Out rule 2 and Result rule 4a, §7.5.1.1.6
/// rules 1 and 4: the `_In`, `_Out`, `_Result`, `_Call` types and the
/// `_Hash` and `_Ex_Hash` constants stand in the namespace of the
/// interface, and not in the global namespace or in another module's.
/// The exception is declared in a third module on purpose.
#[test]
fn the_topic_types_and_constants_stand_in_the_module_of_the_interface() {
    let _native = zerodds_lang_rules::adapter::cpp::CppMappingScope::enter(
        zerodds_lang_rules::adapter::cpp::CppMapping::Native,
    );
    let ast = model(
        "module faults { exception Bad { long why; }; };
         module a { interface Svc { long f(in long x) raises (faults::Bad); }; };
         module b { interface Other { void g(); }; };",
    );
    let decls = interface_cpp::cpp_interface_items(&ast).unwrap_or_else(|e| panic!("{e}"));
    let mut placed = Vec::new();
    cpp_placement(&decls, "", &mut placed);
    let namespace_of = |name: &str| -> Vec<String> {
        placed
            .iter()
            .filter(|(_, declared)| declared == name)
            .map(|(path, _)| path.clone())
            .collect()
    };
    for name in [
        "Svc_f_In",
        "Svc_f_Out",
        "Svc_f_Result",
        "Svc_Call",
        "Svc_Return",
        "Svc_Request",
        "Svc_Reply",
        "Svc_f_Hash",
        "Bad_Ex_Hash",
    ] {
        assert_eq!(namespace_of(name), ["a"], "{name} in {placed:?}");
    }
    for name in ["Other_g_In", "Other_Call", "Other_g_Hash"] {
        assert_eq!(namespace_of(name), ["b"], "{name} in {placed:?}");
    }

    // The Rust binding folds each declaration into the module of its
    // interface as well.
    let pairs = zerodds_lang_rules::emit::interface_rust::rust_interface_pairs(&ast)
        .unwrap_or_else(|error| panic!("{error}"));
    for (module, item) in &pairs {
        let name = match item {
            zerodds_lang_cst::rust::Item::Struct(declared) => declared.name.as_str(),
            zerodds_lang_cst::rust::Item::Enum(declared) => declared.name.as_str(),
            _ => continue,
        };
        let module = ast.module(*module).expect("a module").name.clone();
        if name.starts_with("Svc_") {
            assert_eq!(module, "a", "{name}");
        } else if name.starts_with("Other_") {
            assert_eq!(module, "b", "{name}");
        }
    }
}
