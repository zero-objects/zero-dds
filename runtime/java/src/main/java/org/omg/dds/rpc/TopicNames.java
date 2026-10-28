// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * The names of the request and the reply topic of one service —
 * DDS-RPC 1.0 §7.4.1 and §7.4.2.3.
 *
 * <p>"When more than one method of specifying topic names is used, the
 * run-time specification shall take precedence over the IDL annotations
 * and the default mechanism in that order" (§7.4.2.3). So a run-time topic
 * name suffix wins over everything; a run-time service name wins over an
 * annotated topic name (§7.4.2.2) and over an annotated service name
 * ({@code @service}, {@code @DDSService}); those win over the default
 * {@code "Service"}.
 */
public final class TopicNames {

    /** §7.4.1: the {@code <service_name>} when nothing gives one. */
    public static final String DEFAULT_SERVICE_NAME = "Service";

    private final String request;
    private final String reply;
    private final java.util.List<String> requestAliases;
    private final java.util.List<String> replyAliases;

    private TopicNames(
            String request,
            String reply,
            java.util.List<String> requestAliases,
            java.util.List<String> replyAliases) {
        this.request = request;
        this.reply = reply;
        this.requestAliases = requestAliases;
        this.replyAliases = replyAliases;
    }

    /**
     * The aliases of the request topic (§7.4.3), one per base interface, in order. Empty
     * under the request-reply style, and where a whole topic name was given.
     */
    public java.util.List<String> requestAliases() {
        return requestAliases;
    }

    /** The aliases of the reply topic (§7.4.3). */
    public java.util.List<String> replyAliases() {
        return replyAliases;
    }

    /** The name of the request topic. */
    public String request() {
        return request;
    }

    /** The name of the reply topic. */
    public String reply() {
        return reply;
    }

    /**
     * The names these request parameters give.
     *
     * @param params the parameters of a requester
     * @return the topic names
     * @throws IllegalArgumentException when a name has a character
     *     outside {@code [A-Za-z0-9_ ]}
     */
    public static TopicNames of(RequesterParams params) {
        return resolve(
                params.interfaceName(),
                params.serviceName(),
                params.annotatedServiceName(),
                params.requestTopicNameSuffix(),
                params.replyTopicNameSuffix(),
                params.requestTopicName(),
                params.replyTopicName(),
                params.parents());
    }

    /** As {@link #of(RequesterParams)}, for the parameters of a replier. */
    public static TopicNames of(ReplierParams params) {
        return resolve(
                params.interfaceName(),
                params.serviceName(),
                params.annotatedServiceName(),
                params.requestTopicNameSuffix(),
                params.replyTopicNameSuffix(),
                params.requestTopicName(),
                params.replyTopicName(),
                params.parents());
    }

    /**
     * The names, from the parts the grammar of §7.4.1 names. An empty
     * string stands for "not specified" in every argument.
     *
     * <p>A whole topic name is taken as it is: the generated code of an
     * interface sets the whole names of its two topics, which are not
     * made of the characters §7.4.1 allows in a user-defined part.
     *
     * @param interfaceName the qualified interface name, empty for the
     *     request-reply style (§7.4.1: it has no interface name)
     * @param runtimeServiceName the service name set on the parameters
     * @param annotatedServiceName the service name an annotation gives
     * @param runtimeRequestSuffix the run-time suffix of the request topic
     * @param runtimeReplySuffix the run-time suffix of the reply topic
     * @param annotatedRequestTopic the whole name of the request topic
     *     (§7.4.2.2)
     * @param annotatedReplyTopic the whole name of the reply topic
     * @return the topic names
     */
    public static TopicNames resolve(
            String interfaceName,
            String runtimeServiceName,
            String annotatedServiceName,
            String runtimeRequestSuffix,
            String runtimeReplySuffix,
            String annotatedRequestTopic,
            String annotatedReplyTopic) {
        return resolve(
                interfaceName,
                runtimeServiceName,
                annotatedServiceName,
                runtimeRequestSuffix,
                runtimeReplySuffix,
                annotatedRequestTopic,
                annotatedReplyTopic,
                java.util.Collections.<String>emptyList());
    }

    /**
     * As {@link #resolve(String, String, String, String, String, String, String)}, with the
     * qualified names of the base interfaces, each of which gives one alias of every topic
     * whose name was composed (§7.4.3: an alias is §7.4.1 applied to a base interface; a whole
     * topic name has no interface segment to replace, and the request-reply style has none).
     *
     * @param parents the qualified names of the base interfaces
     * @return the topic names and their aliases
     */
    public static TopicNames resolve(
            String interfaceName,
            String runtimeServiceName,
            String annotatedServiceName,
            String runtimeRequestSuffix,
            String runtimeReplySuffix,
            String annotatedRequestTopic,
            String annotatedReplyTopic,
            java.util.List<String> parents) {
        String service =
                !runtimeServiceName.isEmpty()
                        ? runtimeServiceName
                        : (!annotatedServiceName.isEmpty()
                                ? annotatedServiceName
                                : DEFAULT_SERVICE_NAME);
        validate(service, "service name");
        boolean runtimeService = !runtimeServiceName.isEmpty();
        return new TopicNames(
                one(interfaceName, service, runtimeService, "_Request", runtimeRequestSuffix,
                        annotatedRequestTopic),
                one(interfaceName, service, runtimeService, "_Reply", runtimeReplySuffix,
                        annotatedReplyTopic),
                aliases(interfaceName, parents, service, runtimeService, "_Request",
                        runtimeRequestSuffix, annotatedRequestTopic),
                aliases(interfaceName, parents, service, runtimeService, "_Reply",
                        runtimeReplySuffix, annotatedReplyTopic));
    }

    private static java.util.List<String> aliases(
            String interfaceName,
            java.util.List<String> parents,
            String service,
            boolean runtimeService,
            String defaultSuffix,
            String runtimeSuffix,
            String annotated) {
        if (interfaceName.isEmpty() || parents.isEmpty()) {
            return java.util.Collections.emptyList();
        }
        String suffix;
        if (!runtimeSuffix.isEmpty()) {
            suffix = runtimeSuffix;
        } else if (!annotated.isEmpty() && !runtimeService) {
            return java.util.Collections.emptyList();
        } else {
            suffix = service + defaultSuffix;
        }
        java.util.List<String> out = new java.util.ArrayList<>();
        for (String parent : parents) {
            out.add(compose(parent, suffix));
        }
        return java.util.Collections.unmodifiableList(out);
    }

    private static String one(
            String interfaceName,
            String service,
            boolean runtimeService,
            String defaultSuffix,
            String runtimeSuffix,
            String annotated) {
        if (!runtimeSuffix.isEmpty()) {
            return compose(interfaceName, runtimeSuffix);
        }
        if (!annotated.isEmpty() && !runtimeService) {
            return annotated;
        }
        return compose(interfaceName, service + defaultSuffix);
    }

    private static String compose(String interfaceName, String suffix) {
        validate(suffix, "topic name");
        if (interfaceName.isEmpty()) {
            return suffix;
        }
        validate(interfaceName, "interface name");
        return interfaceName + "_" + suffix;
    }

    /** {@code <user_def_alpha_num> ::= ^[[:alnum:]_ ]+$} — §7.4.1. */
    private static void validate(String text, String what) {
        if (text.isEmpty()) {
            throw new IllegalArgumentException("the " + what + " is empty");
        }
        for (int at = 0; at < text.length(); ++at) {
            char c = text.charAt(at);
            boolean alnum =
                    (c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z');
            if (!alnum && c != '_' && c != ' ') {
                throw new IllegalArgumentException(
                        "the " + what + " \"" + text + "\" has a character outside [A-Za-z0-9_ ]");
            }
        }
    }
}
