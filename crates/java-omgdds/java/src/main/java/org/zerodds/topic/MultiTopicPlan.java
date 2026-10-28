// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import org.omg.dds.topic.Topic;
import org.zerodds.cdr.ReflectionTypeSupport;
import org.zerodds.cdr.ReflectionTypeSupport.MemberRef;
import org.zerodds.sub.ContentFilter;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * What a MultiTopic computes, worked out once from its subscription
 * expression, the topics it reads and the type it produces (DDS-DCPS 1.4
 * §2.2.2.3.4 and Annex B): the columns of the natural join, the output
 * fields with their names, and the check that the result type can hold them.
 *
 * <h2>Row model</h2>
 * A sample of a source topic is a row: the members of its Java class by
 * name (the names of the IDL definition, DDS-Java-PSM 1.0 §8.6). The natural
 * join of several topics is the SQL natural join: a tuple of samples (one
 * of each topic) belongs to the result when every member name the samples
 * share has the same value in all of them, and the joined row holds each
 * such name once, in the order the topics are written in the {@code FROM}
 * clause. Annex B requires the types of the join members to be the same; a
 * pair of topics for which they are not is refused when the MultiTopic is
 * created. Topics that share no member name join as a cross product (the SQL
 * definition), which is the case of the Annex B.2 example.
 *
 * <p>The system property {@code zerodds.multitopic.join} selects the
 * columns: {@code natural} (default, as above) or {@code key}, where only the
 * names that are key members in every topic that has them are compared and
 * the other shared names take the value of the first topic that has them.
 */
final class MultiTopicPlan {

    /** The system property that selects the join columns. */
    static final String JOIN_PROPERTY = "zerodds.multitopic.join";

    /** One output field: where it is read and what it is called in the result. */
    static final class Output {
        final List<String> path;
        final String name;
        final boolean aliased;
        final Class<?> sourceType;

        Output(List<String> path, String name, boolean aliased, Class<?> sourceType) {
            this.path = path;
            this.name = name;
            this.aliased = aliased;
            this.sourceType = sourceType;
        }
    }

    final MultiTopicExpression expression;
    final List<String> topicNames;
    final List<Class<?>> sourceClasses;
    final List<List<MemberRef>> sourceMembers;
    /** The member names of the joined row with their Java types, in row order. */
    final Map<String, Class<?>> columns;
    /** The member names compared by the join. */
    final Set<String> joinColumns;
    final List<Output> outputs;
    final Class<?> resultClass;
    final Map<String, Class<?>> resultMembers;

    private MultiTopicPlan(MultiTopicExpression expression, List<String> topicNames,
            List<Class<?>> sourceClasses, List<List<MemberRef>> sourceMembers,
            Map<String, Class<?>> columns, Set<String> joinColumns,
            List<Output> outputs, Class<?> resultClass,
            Map<String, Class<?>> resultMembers) {
        this.expression = expression;
        this.topicNames = topicNames;
        this.sourceClasses = sourceClasses;
        this.sourceMembers = sourceMembers;
        this.columns = columns;
        this.joinColumns = joinColumns;
        this.outputs = outputs;
        this.resultClass = resultClass;
        this.resultMembers = resultMembers;
    }

    /**
     * @throws IllegalArgumentException if the expression, the topics and the
     *         result type do not fit together (RETCODE_BAD_PARAMETER,
     *         DDS-Java-PSM 1.0 §7.3.2).
     */
    static MultiTopicPlan of(MultiTopicExpression expression,
            List<Topic<?>> sources, Class<?> resultClass,
            List<String> parameters) {
        List<String> names = new ArrayList<>();
        List<Class<?>> classes = new ArrayList<>();
        List<List<MemberRef>> members = new ArrayList<>();
        for (Topic<?> topic : sources) {
            names.add(topic.getName());
            Class<?> type = topic.getTypeSupport().getType();
            classes.add(type);
            members.add(membersOf(type, topic.getName()));
        }

        // The joined row: each name once, in the order of the FROM clause.
        Map<String, Class<?>> columns = new LinkedHashMap<>();
        Map<String, Integer> occurrences = new LinkedHashMap<>();
        Map<String, Boolean> keyEverywhere = new LinkedHashMap<>();
        for (int i = 0; i < members.size(); i++) {
            for (MemberRef member : members.get(i)) {
                Class<?> seen = columns.get(member.getName());
                if (seen == null) {
                    columns.put(member.getName(), member.getType());
                } else if (!seen.equals(member.getType())) {
                    throw new IllegalArgumentException("the topics "
                            + names.get(i) + " and an earlier one of the FROM clause"
                            + " both have the member '" + member.getName()
                            + "' with different types (" + seen.getName() + ", "
                            + member.getType().getName() + "); the members of a"
                            + " NATURAL JOIN must have the same type"
                            + " (DDS-DCPS 1.4 §2.2.2.3.4)");
                }
                occurrences.merge(member.getName(), 1, Integer::sum);
                keyEverywhere.merge(member.getName(), member.isKey(),
                        (a, b) -> a && b);
            }
        }
        Set<String> join = new LinkedHashSet<>();
        boolean keyOnly = "key".equalsIgnoreCase(System.getProperty(JOIN_PROPERTY, "natural"));
        if (!keyOnly && !"natural".equalsIgnoreCase(
                System.getProperty(JOIN_PROPERTY, "natural"))) {
            throw new IllegalArgumentException(JOIN_PROPERTY + " is '"
                    + System.getProperty(JOIN_PROPERTY) + "'; it is 'natural' or 'key'");
        }
        for (Map.Entry<String, Integer> e : occurrences.entrySet()) {
            if (e.getValue() > 1 && (!keyOnly || keyEverywhere.get(e.getKey()))) {
                join.add(e.getKey());
            }
        }

        // The output fields.
        List<Output> outputs = new ArrayList<>();
        Set<String> aliasNames = new LinkedHashSet<>();
        if (expression.isStar()) {
            for (Map.Entry<String, Class<?>> c : columns.entrySet()) {
                outputs.add(new Output(Collections.singletonList(c.getKey()),
                        c.getKey(), false, c.getValue()));
            }
        } else {
            for (MultiTopicExpression.Item item : expression.getItems()) {
                outputs.add(new Output(item.getPath(), item.getName(),
                        item.isAliased(), typeOfPath(columns, item.getPath(),
                                expression.getText())));
                if (item.isAliased()) {
                    aliasNames.add(item.getName());
                }
            }
        }

        // The WHERE clause reads the joined row and the aliases.
        if (expression.getWhere() != null) {
            Set<String> known = new LinkedHashSet<>(columns.keySet());
            known.addAll(aliasNames);
            for (Output o : outputs) {
                known.add(o.name);
            }
            for (List<String> path : ContentFilter.members(expression.getWhere(),
                    parameters)) {
                if (!known.contains(path.get(0))) {
                    throw new IllegalArgumentException("the WHERE clause reads '"
                            + String.join(".", path) + "', but no topic of the"
                            + " FROM clause has a member '" + path.get(0)
                            + "' and no field is called so in the subscription"
                            + " expression '" + expression.getText() + "'");
                }
            }
        }

        // The result type holds every output.
        Map<String, Class<?>> resultMembers = new LinkedHashMap<>();
        for (MemberRef member : membersOf(resultClass, "the result type")) {
            resultMembers.put(member.getName(), member.getType());
        }
        for (Output o : outputs) {
            Class<?> target = resultMembers.get(o.name);
            if (target == null) {
                throw new IllegalArgumentException("the result type "
                        + resultClass.getName() + " has no member '" + o.name
                        + "' for the field '" + String.join(".", o.path)
                        + "' of the subscription expression '"
                        + expression.getText() + "'");
            }
            if (!assignable(o.sourceType, target)) {
                throw new IllegalArgumentException("the field '"
                        + String.join(".", o.path) + "' has the type "
                        + o.sourceType.getName() + ", the member '" + o.name
                        + "' of " + resultClass.getName() + " has the type "
                        + target.getName());
            }
        }
        return new MultiTopicPlan(expression, names, classes, members, columns,
                join, outputs, resultClass, resultMembers);
    }

    private static List<MemberRef> membersOf(Class<?> type, String what) {
        if (type == null || type.isPrimitive() || type.isArray()
                || type.getName().startsWith("java.")
                || org.omg.dds.type.dynamic.DynamicData.class.isAssignableFrom(type)) {
            throw new IllegalArgumentException(what + " has the type "
                    + (type == null ? "null" : type.getName())
                    + ", which has no named members for a MultiTopic to join;"
                    + " a MultiTopic reads topics of Java aggregates"
                    + " (DDS-Java-PSM 1.0 §8.6)");
        }
        return ReflectionTypeSupport.membersOf(type);
    }

    /** The Java type of the field {@code path} of the joined row. */
    private static Class<?> typeOfPath(Map<String, Class<?>> columns,
            List<String> path, String expression) {
        Class<?> type = columns.get(path.get(0));
        if (type == null) {
            throw new IllegalArgumentException("the field '" + path.get(0)
                    + "' is a member of none of the topics in the subscription"
                    + " expression '" + expression + "'");
        }
        for (int i = 1; i < path.size(); i++) {
            Class<?> found = null;
            if (!type.isPrimitive() && !type.isArray() && !type.getName().startsWith("java.")) {
                for (MemberRef member : ReflectionTypeSupport.membersOf(type)) {
                    if (member.getName().equals(path.get(i))) {
                        found = member.getType();
                        break;
                    }
                }
            }
            if (found == null) {
                throw new IllegalArgumentException("'" + String.join(".", path.subList(0, i))
                        + "' has no member '" + path.get(i)
                        + "' in the subscription expression '" + expression + "'");
            }
            type = found;
        }
        return type;
    }

    // --- Values -------------------------------------------------------------

    private static final List<Class<?>> WIDENING = new ArrayList<>();

    static {
        Collections.addAll(WIDENING, byte.class, short.class, int.class,
                long.class, float.class, double.class);
    }

    private static Class<?> boxed(Class<?> type) {
        if (!type.isPrimitive()) {
            return type;
        }
        if (type == int.class) {
            return Integer.class;
        }
        if (type == long.class) {
            return Long.class;
        }
        if (type == short.class) {
            return Short.class;
        }
        if (type == byte.class) {
            return Byte.class;
        }
        if (type == double.class) {
            return Double.class;
        }
        if (type == float.class) {
            return Float.class;
        }
        if (type == boolean.class) {
            return Boolean.class;
        }
        if (type == char.class) {
            return Character.class;
        }
        return Void.class;
    }

    /**
     * Whether a value of {@code from} can be stored in a member of
     * {@code to}: the same type, a numeric widening (byte, short, int, long,
     * float, double in that order), or a subtype.
     */
    static boolean assignable(Class<?> from, Class<?> to) {
        if (boxed(from).equals(boxed(to))) {
            return true;
        }
        int a = WIDENING.indexOf(unboxed(from));
        int b = WIDENING.indexOf(unboxed(to));
        if (a >= 0 && b >= 0) {
            return a <= b;
        }
        return boxed(to).isAssignableFrom(boxed(from));
    }

    private static Class<?> unboxed(Class<?> type) {
        if (type == Integer.class) {
            return int.class;
        }
        if (type == Long.class) {
            return long.class;
        }
        if (type == Short.class) {
            return short.class;
        }
        if (type == Byte.class) {
            return byte.class;
        }
        if (type == Double.class) {
            return double.class;
        }
        if (type == Float.class) {
            return float.class;
        }
        return type;
    }

    /** {@code value} as the exact boxed type of a member of {@code target}. */
    static Object convert(Object value, Class<?> target) {
        if (value == null || !(value instanceof Number)) {
            return value;
        }
        Class<?> t = boxed(target);
        Number n = (Number) value;
        if (t == Integer.class) {
            return n.intValue();
        }
        if (t == Long.class) {
            return n.longValue();
        }
        if (t == Short.class) {
            return n.shortValue();
        }
        if (t == Byte.class) {
            return n.byteValue();
        }
        if (t == Double.class) {
            return n.doubleValue();
        }
        if (t == Float.class) {
            return n.floatValue();
        }
        if (t == BigInteger.class || t == BigDecimal.class) {
            return value;
        }
        return value;
    }
}
