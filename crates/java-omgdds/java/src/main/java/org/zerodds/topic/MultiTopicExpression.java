// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/**
 * The subscription expression of a MultiTopic: the {@code TopicExpression}
 * of DDS-DCPS 1.4 Annex B.2.
 *
 * <pre>
 * TopicExpression ::= SelectFrom { Where } ';'
 * SelectFrom      ::= 'SELECT' Aggregation 'FROM' Selection
 * Aggregation     ::= '*' | ( SubjectFieldSpec // ',' )
 * SubjectFieldSpec::= FIELDNAME | FIELDNAME 'AS' FIELDNAME | FIELDNAME FIELDNAME
 * Selection       ::= TOPICNAME | TOPICNAME NaturalJoin JoinItem
 * JoinItem        ::= TOPICNAME | TOPICNAME NaturalJoin JoinItem
 *                   | '(' TOPICNAME NaturalJoin JoinItem ')'
 * NaturalJoin     ::= 'INNER NATURAL JOIN' | 'NATURAL JOIN' | 'NATURAL INNER JOIN'
 * Where           ::= 'WHERE' Condition
 * </pre>
 *
 * <p>The condition is the filter grammar of Annex B and is compiled by
 * {@link org.zerodds.sub.ContentFilter}; this class only finds where it
 * starts. The closing {@code ;} is accepted and optional: the example of
 * Annex B.2 itself and the {@code subscription_expression} of
 * DDS-Java-PSM 1.0 §7.4.2 are written without it.
 *
 * <p>A {@code TOPICNAME} may be written in single or double quotes, as the
 * Annex B example does ({@code FROM 'Location' NATURAL JOIN 'FlightPlan'}).
 * Annex B names no {@code ORDER BY} for a topic expression (it belongs to the
 * query expression), so a clause of that name is a syntax error here.
 */
public final class MultiTopicExpression {

    /** One item of the projection: a field, read from the joined row, under an output name. */
    public static final class Item {
        private final List<String> path;
        private final String name;
        private final boolean aliased;

        Item(List<String> path, String name, boolean aliased) {
            this.path = Collections.unmodifiableList(path);
            this.name = name;
            this.aliased = aliased;
        }

        /** The {@code FIELDNAME}, split at the dots. */
        public List<String> getPath() {
            return path;
        }

        /**
         * The name of the field in the result type: the alias, or without one
         * the last name of the path (a dotted name selects a nested member,
         * which becomes a top-level member of the result).
         */
        public String getName() {
            return name;
        }

        /** Whether an {@code AS} alias (or the juxtaposed alias) was written. */
        public boolean isAliased() {
            return aliased;
        }
    }

    private final String text;
    private final List<Item> items; // null: '*'
    private final List<String> topics;
    private final String where; // null: no WHERE

    private MultiTopicExpression(String text, List<Item> items, List<String> topics,
            String where) {
        this.text = text;
        this.items = items == null ? null : Collections.unmodifiableList(items);
        this.topics = Collections.unmodifiableList(topics);
        this.where = where;
    }

    /** The expression as it was written. */
    public String getText() {
        return text;
    }

    /** {@code SELECT *}: every field of the joined topics. */
    public boolean isStar() {
        return items == null;
    }

    /** The projection of an explicit field list; empty for {@code *}. */
    public List<Item> getItems() {
        return items == null ? Collections.<Item>emptyList() : items;
    }

    /**
     * The topic names of the {@code FROM} clause in the order they are
     * written. The natural join is associative and commutative, so the join
     * structure (parentheses, the three spellings of the keyword) does not
     * change the result.
     */
    public List<String> getTopics() {
        return topics;
    }

    /** The condition of the {@code WHERE} clause, or {@code null}. */
    public String getWhere() {
        return where;
    }

    /**
     * Parses {@code expression}.
     *
     * @throws IllegalArgumentException if it is not a {@code TopicExpression}
     *         (RETCODE_BAD_PARAMETER, DDS-Java-PSM 1.0 §7.3.2).
     */
    public static MultiTopicExpression parse(String expression) {
        if (expression == null || expression.trim().isEmpty()) {
            throw new IllegalArgumentException(
                    "the subscription expression of a MultiTopic is empty");
        }
        return new Parser(expression).parse();
    }

    @Override
    public String toString() {
        return text;
    }

    // --- The parser ---------------------------------------------------------

    private enum Kind { WORD, QUOTED, COMMA, STAR, LEFT, RIGHT, SEMICOLON }

    private static final class Token {
        final Kind kind;
        final String text;
        final int start;
        final int end;

        Token(Kind kind, String text, int start, int end) {
            this.kind = kind;
            this.text = text;
            this.start = start;
            this.end = end;
        }

        boolean isWord(String keyword) {
            return kind == Kind.WORD && text.equalsIgnoreCase(keyword);
        }
    }

    private static final class Parser {
        private final String source;
        private final List<Token> tokens = new ArrayList<>();
        private int at;

        Parser(String source) {
            this.source = source;
        }

        MultiTopicExpression parse() {
            tokenize();
            expectWord("SELECT");
            List<Item> items = aggregation();
            expectWord("FROM");
            List<String> topics = new ArrayList<>();
            selection(topics);
            String where = null;
            if (peekWord("WHERE")) {
                Token whereToken = tokens.get(at);
                at++;
                int end = source.length();
                // The condition runs to the closing ';' (or to the end).
                for (Token t : tokens.subList(at, tokens.size())) {
                    if (t.kind == Kind.SEMICOLON) {
                        end = t.start;
                        at = tokens.indexOf(t);
                        break;
                    }
                }
                if (end == source.length()) {
                    at = tokens.size();
                }
                where = source.substring(whereToken.end, end).trim();
                if (where.isEmpty()) {
                    throw error("the WHERE clause has no condition");
                }
            }
            if (at < tokens.size() && tokens.get(at).kind == Kind.SEMICOLON) {
                at++;
            }
            if (at < tokens.size()) {
                throw error("'" + describe(tokens.get(at)) + "' is not expected here"
                        + (tokens.get(at).isWord("ORDER")
                                ? " (ORDER BY belongs to a query expression, not to a topic expression)"
                                : ""));
            }
            return new MultiTopicExpression(source, items, topics, where);
        }

        // Aggregation ::= '*' | (SubjectFieldSpec // ',')
        private List<Item> aggregation() {
            if (peekKind(Kind.STAR)) {
                at++;
                return null;
            }
            List<Item> items = new ArrayList<>();
            Set<String> names = new LinkedHashSet<>();
            do {
                if (!items.isEmpty()) {
                    at++; // the comma
                }
                Item item = subjectFieldSpec();
                if (!names.add(item.getName())) {
                    throw error("the field name '" + item.getName()
                            + "' appears twice in the result");
                }
                items.add(item);
            } while (peekKind(Kind.COMMA));
            return items;
        }

        private Item subjectFieldSpec() {
            Token field = next("a field name");
            if (field.kind != Kind.WORD || isKeyword(field.text)) {
                throw error("a field name was expected, got '" + describe(field) + "'");
            }
            List<String> path = fieldPath(field);
            if (peekWord("AS")) {
                at++;
                Token alias = next("an alias");
                if (alias.kind != Kind.WORD || isKeyword(alias.text)
                        || alias.text.indexOf('.') >= 0) {
                    throw error("an alias was expected after AS, got '"
                            + describe(alias) + "'");
                }
                return new Item(path, alias.text, true);
            }
            Token following = at < tokens.size() ? tokens.get(at) : null;
            if (following != null && following.kind == Kind.WORD
                    && !isKeyword(following.text) && following.text.indexOf('.') < 0) {
                at++; // FIELDNAME FIELDNAME
                return new Item(path, following.text, true);
            }
            return new Item(path, path.get(path.size() - 1), false);
        }

        private List<String> fieldPath(Token field) {
            List<String> path = new ArrayList<>();
            for (String part : field.text.split("\\.", -1)) {
                if (part.isEmpty()) {
                    throw error("'" + field.text + "' is not a field name");
                }
                path.add(part);
            }
            return path;
        }

        // Selection ::= TOPICNAME | TOPICNAME NaturalJoin JoinItem
        private void selection(List<String> topics) {
            topics.add(topicName());
            if (peekNaturalJoin()) {
                naturalJoin();
                joinItem(topics);
            }
        }

        // JoinItem ::= TOPICNAME | TOPICNAME NaturalJoin JoinItem
        //            | '(' TOPICNAME NaturalJoin JoinItem ')'
        private void joinItem(List<String> topics) {
            if (peekKind(Kind.LEFT)) {
                at++;
                topics.add(topicName());
                naturalJoin();
                joinItem(topics);
                if (!peekKind(Kind.RIGHT)) {
                    throw error("')' was expected");
                }
                at++;
                return;
            }
            topics.add(topicName());
            if (peekNaturalJoin()) {
                naturalJoin();
                joinItem(topics);
            }
        }

        private String topicName() {
            Token token = next("a topic name");
            String name;
            if (token.kind == Kind.QUOTED) {
                name = token.text;
            } else if (token.kind == Kind.WORD && !isKeyword(token.text)) {
                name = token.text;
            } else {
                throw error("a topic name was expected, got '" + describe(token) + "'");
            }
            if (name.isEmpty() || Character.isDigit(name.charAt(0))) {
                throw error("'" + name + "' is not a topic name");
            }
            for (int i = 0; i < name.length(); i++) {
                char c = name.charAt(i);
                if (!(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z'
                        || c >= '0' && c <= '9' || c == '-' || c == '_')) {
                    throw error("'" + name + "' is not a topic name");
                }
            }
            return name;
        }

        // 'INNER NATURAL JOIN' | 'NATURAL JOIN' | 'NATURAL INNER JOIN'
        private boolean peekNaturalJoin() {
            return peekWord("NATURAL") || peekWord("INNER");
        }

        private void naturalJoin() {
            if (peekWord("INNER")) {
                at++;
                expectWord("NATURAL");
                expectWord("JOIN");
            } else if (peekWord("NATURAL")) {
                at++;
                if (peekWord("INNER")) {
                    at++;
                }
                expectWord("JOIN");
            } else {
                throw error("NATURAL JOIN was expected");
            }
        }

        private static boolean isKeyword(String word) {
            for (String keyword : new String[] {"SELECT", "FROM", "WHERE", "AS",
                    "NATURAL", "INNER", "JOIN"}) {
                if (word.equalsIgnoreCase(keyword)) {
                    return true;
                }
            }
            return false;
        }

        // --- Tokens ---------------------------------------------------------

        private void tokenize() {
            int i = 0;
            while (i < source.length()) {
                char c = source.charAt(i);
                if (Character.isWhitespace(c)) {
                    i++;
                } else if (c == ',') {
                    tokens.add(new Token(Kind.COMMA, ",", i, i + 1));
                    i++;
                } else if (c == '*') {
                    tokens.add(new Token(Kind.STAR, "*", i, i + 1));
                    i++;
                } else if (c == '(') {
                    tokens.add(new Token(Kind.LEFT, "(", i, i + 1));
                    i++;
                } else if (c == ')') {
                    tokens.add(new Token(Kind.RIGHT, ")", i, i + 1));
                    i++;
                } else if (c == ';') {
                    tokens.add(new Token(Kind.SEMICOLON, ";", i, i + 1));
                    i++;
                } else if (c == '\'' || c == '"') {
                    int end = source.indexOf(c, i + 1);
                    if (end < 0) {
                        throw error("a quoted name is not closed");
                    }
                    tokens.add(new Token(Kind.QUOTED, source.substring(i + 1, end),
                            i, end + 1));
                    i = end + 1;
                } else if (Character.isJavaIdentifierStart(c)) {
                    int start = i;
                    while (i < source.length()
                            && (Character.isJavaIdentifierPart(source.charAt(i))
                                || source.charAt(i) == '.' || source.charAt(i) == '-')) {
                        i++;
                    }
                    tokens.add(new Token(Kind.WORD, source.substring(start, i),
                            start, i));
                } else if (inWhere()) {
                    // The condition is not tokenized here: everything from
                    // WHERE on is handed to the filter parser as text.
                    tokens.add(new Token(Kind.WORD, String.valueOf(c), i, i + 1));
                    i++;
                } else {
                    throw error("unexpected character '" + c + "'");
                }
            }
        }

        private boolean inWhere() {
            for (Token t : tokens) {
                if (t.isWord("WHERE")) {
                    return true;
                }
            }
            return false;
        }

        private boolean peekKind(Kind kind) {
            return at < tokens.size() && tokens.get(at).kind == kind;
        }

        private boolean peekWord(String keyword) {
            return at < tokens.size() && tokens.get(at).isWord(keyword);
        }

        private void expectWord(String keyword) {
            if (!peekWord(keyword)) {
                throw error("'" + keyword + "' was expected"
                        + (at < tokens.size()
                                ? ", got '" + describe(tokens.get(at)) + "'" : ""));
            }
            at++;
        }

        private Token next(String what) {
            if (at >= tokens.size()) {
                throw error(what + " was expected");
            }
            return tokens.get(at++);
        }

        private static String describe(Token token) {
            return token.kind == Kind.QUOTED ? "'" + token.text + "'" : token.text;
        }

        private IllegalArgumentException error(String what) {
            return new IllegalArgumentException(what
                    + " in the subscription expression '" + source + "'");
        }
    }
}
