// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Synthesizing DDS topic names — Spec §7.4.1.
//!
//! ```text
//! <topic_name>        ::= <interface_name> "_" <service_name> "_" [ "Request" | "Reply" ]
//!                       | <user_def_alpha_num>
//! <service_name>      ::= "Service"
//!                       | <user_def_alpha_num>
//! <user_def_alpha_num> ::= ^[[:alnum:]_ ]+$
//! ```
//!
//! Three things in that grammar decide a name, and getting any of them
//! wrong means a conforming peer does not find us.
//!
//! **`<interface_name>` is the fully-qualified name.** §7.4.1: "The
//! `<interface_name>` non-terminal represents the fully-qualified name
//! of the interface […] A fully qualified interface-name includes
//! concatenation of module names separates by underscores followed by
//! the name of the interface." So `module robot { interface Calculator }`
//! contributes `robot_Calculator`, not `Calculator`.
//!
//! **`<service_name>` defaults to `"Service"`.** §7.4.1: "The
//! `<service_name>` non-terminal may be user-supplied. When it is not,
//! it defaults to 'Service.'" The default topic of a `Calculator` is
//! therefore `Calculator_Service_Request` — the non-normative table in
//! §7.4.3 prints exactly that — and never `Calculator_Request`.
//!
//! **The interface segment is captured for one binding style and not the
//! other.** §7.4.1, last paragraph: "For the request-reply style
//! language binding, `<interface_name>` and the following underscore
//! shall not be captured automatically." §7.4.3 repeats it. So the same
//! service has two default names depending on which binding the
//! application uses, and [`BindingStyle`] is what says which — see
//! §7.4.2: "It is possible to use different mechanism at the client and
//! server sides. However, to ensure successful end-point matching, the
//! topic names must match."
//!
//! ## The three sources and their order
//!
//! §7.4.2.3 and §7.4.3.3, in the same words: "When more than one method
//! of specifying topic names is used, the run-time specification shall
//! take precedence over the IDL annotations and the default mechanism in
//! that order." [`TopicNameOverrides`] is the run-time layer,
//! `ServiceDef::request_topic` / `reply_topic` the annotation layer
//! (§7.4.2.2's `@DDSRequestTopic` / `@DDSReplyTopic`), and the
//! composition above the default.
//!
//! The run-time layer specifies *a service name and a topic suffix* —
//! §7.4.2.3: "provide functions to specify the service name and the
//! topic name suffix. When used as such, the rules defined in 7.4.1 […]
//! apply." — so it feeds the composition rather than replacing it. The
//! annotation gives a whole name, which is §7.4.1's second alternative
//! `<topic_name> ::= <user_def_alpha_num>`.
//!
//! ## Aliases
//!
//! §7.4.3: "In case of single and multiple inheritance, the request and
//! reply topics shall additionally have topic-aliases, which are
//! obtained using the rules in sub clause 7.4.1 for each parent
//! interface." An alias varies the interface segment and keeps the
//! suffix, which is why a whole-name annotation produces none: there is
//! no interface segment in it to vary.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::{RpcError, RpcResult};

/// Topic suffix for request topics (Spec §7.4.1).
pub const REQUEST_SUFFIX: &str = "_Request";

/// Topic suffix for reply topics (Spec §7.4.1).
pub const REPLY_SUFFIX: &str = "_Reply";

/// The `<service_name>` a service takes when the user supplies none.
///
/// §7.4.1: "When it is not, it defaults to 'Service.'"
pub const DEFAULT_SERVICE_NAME: &str = "Service";

/// Which language binding is asking for the name. Spec §7.4.1.
///
/// Not a property of the service: the same IDL interface has one default
/// topic name under the function-call binding and another under the
/// request-reply binding, because the clause says the interface segment
/// "shall not be captured automatically" for the second. A generator
/// that picked one for both would produce a client and a service that
/// never match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BindingStyle {
    /// §7.11.1.1.2 — the interface segment is captured.
    #[default]
    FunctionCall,
    /// §7.11.1.1.1 — it is not.
    RequestReply,
}

impl BindingStyle {
    /// Both styles, for a test that has to cover the pair.
    pub const ALL: [Self; 2] = [Self::FunctionCall, Self::RequestReply];

    /// Whether this style prefixes `<interface_name>` and an underscore.
    #[must_use]
    pub const fn captures_interface_name(self) -> bool {
        matches!(self, Self::FunctionCall)
    }
}

/// What §7.4.2.3's `ServiceParams`, `ClientParams`, `RequesterParams`
/// and `ReplierParams` carry: "functions to specify the service name and
/// the topic name suffix".
///
/// Every field absent is the empty override, which is what a caller that
/// specifies nothing at run time passes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopicNameOverrides {
    /// The run-time `<service_name>`, replacing the annotation's and the
    /// `"Service"` default.
    pub service_name: Option<String>,
    /// The run-time request topic suffix, replacing the whole
    /// `<service_name> "_" "Request"` tail.
    pub request_suffix: Option<String>,
    /// The run-time reply topic suffix.
    pub reply_suffix: Option<String>,
}

impl TopicNameOverrides {
    /// Nothing specified at run time.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Only the service name, which is the common case: a second
    /// instance of one service on one domain.
    #[must_use]
    pub fn with_service_name(name: impl Into<String>) -> Self {
        Self {
            service_name: Some(name.into()),
            ..Self::default()
        }
    }

    /// Whether anything at all was specified.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.service_name.is_none() && self.request_suffix.is_none() && self.reply_suffix.is_none()
    }
}

/// Validates a `<user_def_alpha_num>`. Spec §7.4.1.
///
/// The published regex is `^[[:alnum:]_ ]+$`, and the clause spells the
/// set out in words directly under it: "a regular expression that
/// includes alphanumeric characters (a-zA-Z0-9), underscore and space."
/// Space is in the set — a DDS topic name may carry one — and there is
/// no rule about the first character: `<user_def_alpha_num>` is not an
/// IDL identifier and the grammar does not make it one.
///
/// # Errors
/// `RpcError::InvalidServiceName` for an empty string or a character
/// outside `[A-Za-z0-9_ ]`.
pub fn validate_user_def_alpha_num(name: &str) -> RpcResult<()> {
    if name.is_empty() {
        return Err(RpcError::InvalidServiceName(String::new()));
    }
    for &b in name.as_bytes() {
        if !(b.is_ascii_alphanumeric() || b == b'_' || b == b' ') {
            return Err(RpcError::InvalidServiceName(name.into()));
        }
    }
    Ok(())
}

/// Validates a service name. The same production as
/// [`validate_user_def_alpha_num`]: §7.4.1 defines `<service_name>` as
/// `"Service" | <user_def_alpha_num>`, and `"Service"` matches the
/// second alternative anyway.
///
/// # Errors
/// See [`validate_user_def_alpha_num`].
pub fn validate_service_name(service: &str) -> RpcResult<()> {
    validate_user_def_alpha_num(service)
}

/// The `<interface_name>` of §7.4.1 for one interface: "concatenation of
/// module names separates by underscores followed by the name of the
/// interface".
///
/// The anonymous global scope contributes no segment, so an interface at
/// file scope is its own qualified name.
#[must_use]
pub fn qualified_interface_name(modules: &[&str], interface: &str) -> String {
    let mut out = String::new();
    for module in modules {
        if module.is_empty() {
            continue;
        }
        out.push_str(module);
        out.push('_');
    }
    out.push_str(interface);
    out
}

/// One topic name, composed from the parts §7.4.1 names.
///
/// `suffix` is the whole tail after the interface segment — either
/// `<service_name> "_" "Request"` or a user-supplied topic suffix.
///
/// # Errors
/// `RpcError::InvalidServiceName` when a part is not a
/// `<user_def_alpha_num>`.
pub fn compose_topic_name(
    style: BindingStyle,
    qualified_interface: &str,
    suffix: &str,
) -> RpcResult<String> {
    validate_user_def_alpha_num(suffix)?;
    if !style.captures_interface_name() {
        return Ok(suffix.to_string());
    }
    validate_user_def_alpha_num(qualified_interface)?;
    Ok(format!("{qualified_interface}_{suffix}"))
}

/// The two topic names of one service, with the aliases its parent
/// interfaces contribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceTopicNames {
    /// The `<service_name>` in effect, after the precedence of §7.4.2.3.
    pub service: String,
    /// The request topic.
    pub request: String,
    /// The reply topic.
    pub reply: String,
    /// §7.4.3's request-topic aliases, one per parent interface, in
    /// declaration order. Empty where the interface has no parents, or
    /// where the name came from an annotation whole-name.
    pub request_aliases: Vec<String>,
    /// §7.4.3's reply-topic aliases.
    pub reply_aliases: Vec<String>,
}

impl ServiceTopicNames {
    /// The names of an interface with no parents and nothing overridden,
    /// under the function-call binding.
    ///
    /// # Errors
    /// `RpcError::InvalidServiceName` on a name §7.4.1 does not admit.
    pub fn new(qualified_interface: &str) -> RpcResult<Self> {
        Self::synthesize(
            BindingStyle::FunctionCall,
            qualified_interface,
            &[],
            DEFAULT_SERVICE_NAME,
            None,
            None,
            &TopicNameOverrides::none(),
        )
    }

    /// The full composition of §7.4.1 with the precedence of §7.4.2.3.
    ///
    /// `annotated_request` / `annotated_reply` are §7.4.2.2's
    /// `@DDSRequestTopic` / `@DDSReplyTopic` values — whole topic names,
    /// §7.4.1's second alternative. `overrides` is the run-time layer
    /// and beats them.
    ///
    /// # Errors
    /// `RpcError::InvalidServiceName` when any part is not a
    /// `<user_def_alpha_num>`.
    pub fn synthesize(
        style: BindingStyle,
        qualified_interface: &str,
        parents: &[&str],
        service_name: &str,
        annotated_request: Option<&str>,
        annotated_reply: Option<&str>,
        overrides: &TopicNameOverrides,
    ) -> RpcResult<Self> {
        // §7.4.2.3: the run-time service name beats the annotated one,
        // which beats `"Service"`.
        let effective_service = overrides.service_name.as_deref().unwrap_or(service_name);
        validate_service_name(effective_service)?;

        let request = Self::one(
            style,
            qualified_interface,
            parents,
            effective_service,
            REQUEST_SUFFIX,
            overrides.request_suffix.as_deref(),
            annotated_request,
            overrides.service_name.is_some(),
        )?;
        let reply = Self::one(
            style,
            qualified_interface,
            parents,
            effective_service,
            REPLY_SUFFIX,
            overrides.reply_suffix.as_deref(),
            annotated_reply,
            overrides.service_name.is_some(),
        )?;

        Ok(Self {
            service: effective_service.to_string(),
            request: request.0,
            reply: reply.0,
            request_aliases: request.1,
            reply_aliases: reply.1,
        })
    }

    /// One side's topic name and its aliases.
    #[allow(clippy::too_many_arguments)]
    fn one(
        style: BindingStyle,
        qualified_interface: &str,
        parents: &[&str],
        service_name: &str,
        default_suffix: &str,
        runtime_suffix: Option<&str>,
        annotated: Option<&str>,
        runtime_service_name: bool,
    ) -> RpcResult<(String, Vec<String>)> {
        // 1. A run-time topic suffix. §7.4.3's table calls the result
        //    `${interface}_${topic-suffix}`, so the suffix replaces the
        //    whole `<service_name>_Request` tail and the interface
        //    segment stays.
        if let Some(suffix) = runtime_suffix {
            return Ok((
                compose_topic_name(style, qualified_interface, suffix)?,
                Self::aliases(style, parents, suffix)?,
            ));
        }
        // 2. A whole topic name from the annotation, unless the run time
        //    supplied a service name — §7.4.2.3 puts the run-time layer
        //    above the annotation, and a run-time service name is a
        //    run-time specification of the topic name.
        if let Some(name) = annotated.filter(|_| !runtime_service_name) {
            validate_user_def_alpha_num(name)?;
            // No aliases: §7.4.3 obtains an alias by applying §7.4.1 to
            // a parent interface, and a whole name has no interface
            // segment to replace. Inventing `Parent_<name>` would
            // announce a topic nobody publishes.
            return Ok((name.to_string(), Vec::new()));
        }
        // 3. The composition, with `<service_name>` already resolved.
        let suffix = format!("{service_name}{default_suffix}");
        Ok((
            compose_topic_name(style, qualified_interface, &suffix)?,
            Self::aliases(style, parents, &suffix)?,
        ))
    }

    /// §7.4.3: one alias per parent interface, same suffix.
    ///
    /// None under the request-reply binding: the name carries no
    /// interface segment there, so every parent would produce the same
    /// string as the topic itself.
    fn aliases(style: BindingStyle, parents: &[&str], suffix: &str) -> RpcResult<Vec<String>> {
        if !style.captures_interface_name() {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(parents.len());
        for parent in parents {
            out.push(compose_topic_name(style, parent, suffix)?);
        }
        Ok(out)
    }
}

/// Everything a service can declare, beyond its own qualified interface
/// name, to pick its topic pair — DDS-RPC 1.0 §7.4.2.2's
/// `@service`/`@DDSRequestTopic`/`@DDSReplyTopic` annotation values, and
/// §7.4.2.3's run-time layer on top of them. `Requester::with_topics`
/// and `Replier::with_topics` (§7.9/§7.10) both take one — the same
/// three inputs a service declares, so a generated binding that reads
/// its own IDL's annotations has one place to hand them to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotatedTopics {
    /// `@service(name = "...")`. `None` is "the interface declares no
    /// service name", which §7.4.1 defaults to `"Service"` — not the
    /// same as an actual empty string, which [`validate_service_name`]
    /// rejects rather than silently falling back to the default.
    pub service_name: Option<String>,
    /// `@DDSRequestTopic("...")` (§7.4.2.2): a whole request-topic name.
    pub request_topic: Option<String>,
    /// `@DDSReplyTopic("...")` (§7.4.2.2): a whole reply-topic name.
    pub reply_topic: Option<String>,
    /// The run-time layer (§7.4.2.3): a service name and/or topic
    /// suffixes supplied at run time, which take precedence over the
    /// two annotation fields above and over the default.
    pub overrides: TopicNameOverrides,
}

impl AnnotatedTopics {
    /// No annotation and no run-time override — the plain default,
    /// `<interface>_Service_Request`/`_Reply`.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// The full composition: §7.4.1's grammar, under §7.4.2.3's
    /// precedence (run time, then annotation, then default).
    ///
    /// # Errors
    /// `RpcError::InvalidServiceName` when a part is not a
    /// `<user_def_alpha_num>`.
    pub fn resolve(
        &self,
        style: BindingStyle,
        qualified_interface: &str,
        parents: &[&str],
    ) -> RpcResult<ServiceTopicNames> {
        let service_name = self.service_name.as_deref().unwrap_or(DEFAULT_SERVICE_NAME);
        ServiceTopicNames::synthesize(
            style,
            qualified_interface,
            parents,
            service_name,
            self.request_topic.as_deref(),
            self.reply_topic.as_deref(),
            &self.overrides,
        )
    }
}

/// Returns the request topic name for a service under the function-call
/// binding, with `<service_name>` at its default.
///
/// # Errors
/// See [`validate_service_name`].
pub fn request_topic_name(qualified_interface: &str) -> RpcResult<String> {
    validate_user_def_alpha_num(qualified_interface)?;
    compose_topic_name(
        BindingStyle::FunctionCall,
        qualified_interface,
        &format!("{DEFAULT_SERVICE_NAME}{REQUEST_SUFFIX}"),
    )
}

/// Returns the reply topic name for a service under the function-call
/// binding, with `<service_name>` at its default.
///
/// # Errors
/// See [`validate_service_name`].
pub fn reply_topic_name(qualified_interface: &str) -> RpcResult<String> {
    validate_user_def_alpha_num(qualified_interface)?;
    compose_topic_name(
        BindingStyle::FunctionCall,
        qualified_interface,
        &format!("{DEFAULT_SERVICE_NAME}{REPLY_SUFFIX}"),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// §7.4.3's non-normative table, "Everything Default" row: the
    /// request topic of a `Calculator` is `Calculator_Service_Request`.
    #[test]
    fn the_default_name_carries_the_service_segment() {
        assert_eq!(
            request_topic_name("Calculator").unwrap(),
            "Calculator_Service_Request"
        );
        assert_eq!(
            reply_topic_name("Calculator").unwrap(),
            "Calculator_Service_Reply"
        );
    }

    /// §7.4.1: "A fully qualified interface-name includes concatenation
    /// of module names separates by underscores followed by the name of
    /// the interface."
    #[test]
    fn the_interface_segment_is_the_fully_qualified_name() {
        assert_eq!(
            qualified_interface_name(&["robot"], "Calculator"),
            "robot_Calculator"
        );
        assert_eq!(qualified_interface_name(&["a", "b"], "I"), "a_b_I");
        assert_eq!(qualified_interface_name(&[], "I"), "I");
        assert_eq!(
            request_topic_name(&qualified_interface_name(&["robot"], "Calculator")).unwrap(),
            "robot_Calculator_Service_Request"
        );
    }

    /// §7.4.1: "For the request-reply style language binding,
    /// `<interface_name>` and the following underscore shall not be
    /// captured automatically."
    #[test]
    fn the_request_reply_style_drops_the_interface_segment() {
        let names = ServiceTopicNames::synthesize(
            BindingStyle::RequestReply,
            "robot_Calculator",
            &[],
            DEFAULT_SERVICE_NAME,
            None,
            None,
            &TopicNameOverrides::none(),
        )
        .unwrap();
        assert_eq!(names.request, "Service_Request");
        assert_eq!(names.reply, "Service_Reply");
        assert!(names.request_aliases.is_empty());
    }

    /// §7.4.1 and §7.4.3: "For the request-reply style language binding,
    /// the interfaces name shall not be annexed automatically." With
    /// parent interfaces the aliases would be the parents' names
    /// annexed, so there are none, and the topic names stay the bare
    /// `Service_Request` and `Service_Reply` whatever the bases are.
    #[test]
    fn the_request_reply_style_has_no_aliases_for_the_bases_either() {
        let names = ServiceTopicNames::synthesize(
            BindingStyle::RequestReply,
            "robot_Calculator",
            &["robot_Adder", "robot_Subtractor"],
            DEFAULT_SERVICE_NAME,
            None,
            None,
            &TopicNameOverrides::none(),
        )
        .unwrap();
        assert_eq!(names.request, "Service_Request");
        assert_eq!(names.reply, "Service_Reply");
        assert!(
            names.request_aliases.is_empty(),
            "{:?}",
            names.request_aliases
        );
        assert!(names.reply_aliases.is_empty(), "{:?}", names.reply_aliases);
        // The same bases under the function-call style do alias.
        let called = ServiceTopicNames::synthesize(
            BindingStyle::FunctionCall,
            "robot_Calculator",
            &["robot_Adder", "robot_Subtractor"],
            DEFAULT_SERVICE_NAME,
            None,
            None,
            &TopicNameOverrides::none(),
        )
        .unwrap();
        assert_eq!(called.request_aliases.len(), 2);
        // A user service name and the bases: the service segment changes,
        // the missing interface segment and the missing aliases stay.
        let named = ServiceTopicNames::synthesize(
            BindingStyle::RequestReply,
            "robot_Calculator",
            &["robot_Adder"],
            "Arithmetic",
            None,
            None,
            &TopicNameOverrides::none(),
        )
        .unwrap();
        assert_eq!(named.request, "Arithmetic_Request");
        assert!(named.request_aliases.is_empty());
        // A run-time suffix in the request-reply style is the whole name.
        let suffixed = ServiceTopicNames::synthesize(
            BindingStyle::RequestReply,
            "robot_Calculator",
            &["robot_Adder"],
            DEFAULT_SERVICE_NAME,
            None,
            None,
            &TopicNameOverrides {
                request_suffix: Some("Calls".to_string()),
                ..TopicNameOverrides::none()
            },
        )
        .unwrap();
        assert_eq!(suffixed.request, "Calls");
        assert!(suffixed.request_aliases.is_empty());
    }

    /// §7.4.3's table, "User-defined service name" row.
    #[test]
    fn a_user_supplied_service_name_replaces_only_that_segment() {
        let names = ServiceTopicNames::synthesize(
            BindingStyle::FunctionCall,
            "Calculator",
            &[],
            "Arithmetic",
            None,
            None,
            &TopicNameOverrides::none(),
        )
        .unwrap();
        assert_eq!(names.request, "Calculator_Arithmetic_Request");
        assert_eq!(names.reply, "Calculator_Arithmetic_Reply");
        assert_eq!(names.service, "Arithmetic");
    }

    /// §7.4.3's table, "User-defined topic suffixes" row:
    /// `Calculator_${topic-suffix-request}`.
    #[test]
    fn a_user_supplied_suffix_replaces_the_whole_tail() {
        let names = ServiceTopicNames::synthesize(
            BindingStyle::FunctionCall,
            "Calculator",
            &["Adder"],
            DEFAULT_SERVICE_NAME,
            None,
            None,
            &TopicNameOverrides {
                service_name: None,
                request_suffix: Some("Calls".to_string()),
                reply_suffix: Some("Answers".to_string()),
            },
        )
        .unwrap();
        assert_eq!(names.request, "Calculator_Calls");
        assert_eq!(names.reply, "Calculator_Answers");
        assert_eq!(names.request_aliases, ["Adder_Calls"]);
        assert_eq!(names.reply_aliases, ["Adder_Answers"]);
    }

    /// §7.4.3's table, alias column of the "Everything Default" row:
    /// `Adder_Service_Request`, `Subtractor_Service_Request`.
    #[test]
    fn every_parent_interface_contributes_one_alias() {
        let names = ServiceTopicNames::synthesize(
            BindingStyle::FunctionCall,
            "Calculator",
            &["Adder", "Subtractor"],
            DEFAULT_SERVICE_NAME,
            None,
            None,
            &TopicNameOverrides::none(),
        )
        .unwrap();
        assert_eq!(names.request, "Calculator_Service_Request");
        assert_eq!(
            names.request_aliases,
            ["Adder_Service_Request", "Subtractor_Service_Request"]
        );
        assert_eq!(
            names.reply_aliases,
            ["Adder_Service_Reply", "Subtractor_Service_Reply"]
        );
    }

    /// §7.4.2.2: the annotation gives a whole topic name — §7.4.1's
    /// second alternative `<topic_name> ::= <user_def_alpha_num>`.
    #[test]
    fn an_annotation_gives_the_whole_name_and_no_alias() {
        let names = ServiceTopicNames::synthesize(
            BindingStyle::FunctionCall,
            "Calculator",
            &["Adder"],
            DEFAULT_SERVICE_NAME,
            Some("MyReqTopic"),
            Some("MyRepTopic"),
            &TopicNameOverrides::none(),
        )
        .unwrap();
        assert_eq!(names.request, "MyReqTopic");
        assert_eq!(names.reply, "MyRepTopic");
        assert!(names.request_aliases.is_empty());
        assert!(names.reply_aliases.is_empty());
    }

    /// §7.4.2.3: "the run-time specification shall take precedence over
    /// the IDL annotations and the default mechanism in that order."
    #[test]
    fn the_run_time_specification_beats_the_annotation() {
        let names = ServiceTopicNames::synthesize(
            BindingStyle::FunctionCall,
            "Calculator",
            &[],
            DEFAULT_SERVICE_NAME,
            Some("MyReqTopic"),
            Some("MyRepTopic"),
            &TopicNameOverrides::with_service_name("Live"),
        )
        .unwrap();
        assert_eq!(names.request, "Calculator_Live_Request");
        assert_eq!(names.reply, "Calculator_Live_Reply");

        let suffixed = ServiceTopicNames::synthesize(
            BindingStyle::FunctionCall,
            "Calculator",
            &[],
            "Arithmetic",
            Some("MyReqTopic"),
            None,
            &TopicNameOverrides {
                service_name: None,
                request_suffix: Some("Now".to_string()),
                reply_suffix: None,
            },
        )
        .unwrap();
        assert_eq!(suffixed.request, "Calculator_Now");
        // Nothing at run time for the reply, so the annotation stands.
        assert_eq!(suffixed.reply, "Calculator_Arithmetic_Reply");
    }

    /// §7.4.1's `<user_def_alpha_num>` is `^[[:alnum:]_ ]+$` — the
    /// clause names the space in words directly under the grammar.
    #[test]
    fn the_published_character_set_admits_a_space_and_a_leading_digit() {
        assert!(validate_user_def_alpha_num("My Service").is_ok());
        assert!(validate_user_def_alpha_num("9calc").is_ok());
        assert!(validate_user_def_alpha_num("_foo").is_ok());
        assert!(validate_user_def_alpha_num("Calc2_v3").is_ok());
    }

    #[test]
    fn everything_outside_the_set_is_refused() {
        for name in ["", "my-service", "Rechnerü", "a.b", "a\tb"] {
            assert!(
                validate_user_def_alpha_num(name).is_err(),
                "{name:?} is not a <user_def_alpha_num>"
            );
        }
        assert_eq!(
            validate_user_def_alpha_num("").unwrap_err(),
            RpcError::InvalidServiceName(String::new())
        );
    }

    #[test]
    fn service_topic_names_pair() {
        let names = ServiceTopicNames::new("Calc").unwrap();
        assert_eq!(names.service, "Service");
        assert_eq!(names.request, "Calc_Service_Request");
        assert_eq!(names.reply, "Calc_Service_Reply");
    }

    #[test]
    fn service_topic_names_propagates_error() {
        let err = ServiceTopicNames::new("").unwrap_err();
        assert!(matches!(err, RpcError::InvalidServiceName(_)));
    }

    /// Both styles are covered by `ALL`, so a third could not be added
    /// without a test noticing.
    #[test]
    fn every_binding_style_composes_a_name() {
        for style in BindingStyle::ALL {
            let name = compose_topic_name(style, "I", "Service_Request").unwrap();
            assert!(name.ends_with("Service_Request"), "{name}");
            assert_eq!(
                name.starts_with("I_"),
                style.captures_interface_name(),
                "{style:?}"
            );
        }
    }
}
