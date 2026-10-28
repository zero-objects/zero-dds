// SPDX-License-Identifier: Apache-2.0
package org.zerodds.sub;

import java.lang.reflect.AccessibleObject;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Predicate;

/**
 * The content filter expressions of DDS-DCPS 1.4 §2.2.2.5.3.6 / DDSI-RTPS 2.5
 * §9.4.1.1, compiled into a predicate over a deserialized sample.
 *
 * <p>The same expression drives three operations of this binding:
 * {@code createContentFilteredTopic} (§2.2.2.3.3), which filters the samples
 * a reader receives, {@code createQueryCondition} (§2.2.2.5.3.6), whose
 * trigger value asks whether a matching sample exists, and the query
 * expression of a read/take {@code Selector} (§7.7.3).
 *
 * <p>The grammar is the one of DDSI-RTPS 2.5 §9.4.1.1:
 *
 * <pre>
 * expression ::= expression 'AND' expression
 *              | expression 'OR'  expression
 *              | 'NOT' expression
 *              | '(' expression ')'
 *              | predicate
 * predicate  ::= parameter_id '='  literal | parameter_id '!=' literal
 *              | parameter_id '&lt;'  literal | parameter_id '&lt;=' literal
 *              | parameter_id '&gt;'  literal | parameter_id '&gt;=' literal
 *              | parameter_id LIKE string [ 'ESCAPE' char ]
 *              | parameter_id BETWEEN literal 'AND' literal
 *              | parameter_id IN '(' literal ( ',' literal )* ')'
 *              | parameter_id IS [ 'NOT' ] NULL
 *              | parameter_index
 * </pre>
 *
 * <p>A {@code parameter_id} is a dotted path of member names, resolved on
 * the sample through its accessors — {@code getX()}, {@code isX()}, a
 * no-argument method named like the member, or a field — so both Java beans
 * and records work. A {@code parameter_index} ({@code %n} or a bare number)
 * stands for the n-th element of the expression parameter list; the
 * parameter text is read as a literal of the kind its first character or its
 * spelling says.
 */
public final class ContentFilter {

    private final String expression;
    private final List<String> parameters;
    private final List<Token> tokens;
    private final List<List<String>> referenced = new ArrayList<>();
    private int at;

    private ContentFilter(String expression, List<String> parameters) {
        this.expression = expression;
        this.parameters = parameters == null ? java.util.Collections.emptyList() : parameters;
        this.tokens = tokenize(expression);
    }

    /**
     * Compiles {@code expression} with {@code parameters} into a predicate
     * over a sample.
     *
     * <p>An empty expression matches every sample (DDSI-RTPS 2.5 §9.4.1.1:
     * an empty filter expression matches everything).
     *
     * @throws IllegalArgumentException if the expression does not parse or
     *         names a parameter that is not in the list (RETCODE_BAD_PARAMETER,
     *         DDS-Java-PSM 1.0 §7.3.2, Table 7.1).
     */
    @SuppressWarnings("unchecked")
    public static <T> Predicate<T> compile(String expression,
            List<String> parameters) {
        if (expression == null || expression.trim().isEmpty()) {
            return sample -> true;
        }
        ContentFilter filter = new ContentFilter(expression, parameters);
        Predicate<Object> compiled = filter.expression();
        // expectEnd catches the rest of the expression, which a
        // recursive-descent parser would otherwise leave unread.
        filter.expectEnd();
        return (Predicate<T>) compiled;
    }

    /**
     * The member paths {@code expression} reads, in order of appearance and
     * without repeats. A path is the list of the names of a dotted
     * {@code FIELDNAME} (DDS-DCPS 1.4 Annex B). The expression is parsed as
     * {@link #compile} parses it and fails the same way.
     */
    public static List<List<String>> members(String expression,
            List<String> parameters) {
        if (expression == null || expression.trim().isEmpty()) {
            return new ArrayList<>();
        }
        ContentFilter filter = new ContentFilter(expression, parameters);
        filter.expression();
        filter.expectEnd();
        List<List<String>> distinct = new ArrayList<>();
        for (List<String> path : filter.referenced) {
            if (!distinct.contains(path)) {
                distinct.add(path);
            }
        }
        return distinct;
    }

    // --- The parser ---------------------------------------------------------

    private Predicate<Object> expression() {
        return or();
    }

    private Predicate<Object> or() {
        Predicate<Object> left = and();
        while (peekKeyword("OR")) {
            next();
            Predicate<Object> right = and();
            Predicate<Object> current = left;
            left = sample -> current.test(sample) || right.test(sample);
        }
        return left;
    }

    private Predicate<Object> and() {
        Predicate<Object> left = not();
        while (peekKeyword("AND")) {
            next();
            Predicate<Object> right = not();
            Predicate<Object> current = left;
            left = sample -> current.test(sample) && right.test(sample);
        }
        return left;
    }

    private Predicate<Object> not() {
        if (peekKeyword("NOT")) {
            next();
            Predicate<Object> inner = not();
            return sample -> !inner.test(sample);
        }
        return primary();
    }

    private Predicate<Object> primary() {
        Token token = peek();
        if (token == null) {
            throw error("the expression ends where a predicate was expected");
        }
        if (token.type == Type.LEFT_PAREN) {
            next();
            Predicate<Object> inner = expression();
            expect(Type.RIGHT_PAREN, "')'");
            return inner;
        }
        if (token.type == Type.PARAMETER_INDEX) {
            next();
            if (isComparison(peek())) {
                // Annex B: Parameter RelOp FIELDNAME.
                return literalFirst(parameter(token.index));
            }
            return parameterPredicate(token);
        }
        if (token.type == Type.NUMBER || token.type == Type.STRING
                || token.type == Type.BOOLEAN) {
            // Annex B: Parameter RelOp FIELDNAME, with the parameter given as
            // a literal.
            return literalFirst(literal());
        }
        if (token.type != Type.IDENTIFIER) {
            throw error("a member name or a parameter index was expected, got '"
                    + token.text + "'");
        }
        List<String> path = memberPath();
        return predicate(path);
    }

    private List<String> memberPath() {
        List<String> path = new ArrayList<>();
        path.add(next().text);
        while (peekType(Type.DOT)) {
            next();
            path.add(expect(Type.IDENTIFIER, "a member name").text);
        }
        referenced.add(path);
        return path;
    }

    private static boolean isComparison(Token token) {
        if (token == null) {
            return false;
        }
        switch (token.type) {
            case EQUAL:
            case NOT_EQUAL:
            case LESS:
            case LESS_OR_EQUAL:
            case GREATER:
            case GREATER_OR_EQUAL:
                return true;
            default:
                return false;
        }
    }

    /** {@code literal RelOp FIELDNAME}: the field compared with the literal, mirrored. */
    private Predicate<Object> literalFirst(Object literal) {
        Token operator = expectOneOf("a comparison operator", Type.EQUAL,
                Type.NOT_EQUAL, Type.LESS, Type.LESS_OR_EQUAL, Type.GREATER,
                Type.GREATER_OR_EQUAL);
        Token field = peek();
        if (field == null || field.type != Type.IDENTIFIER) {
            throw error("a member name was expected after the comparison");
        }
        List<String> path = memberPath();
        return comparison(path, mirror(operator.type), literal);
    }

    /** The operator that gives the same answer with the operands exchanged. */
    private static Type mirror(Type operator) {
        switch (operator) {
            case LESS:
                return Type.GREATER;
            case LESS_OR_EQUAL:
                return Type.GREATER_OR_EQUAL;
            case GREATER:
                return Type.LESS;
            case GREATER_OR_EQUAL:
                return Type.LESS_OR_EQUAL;
            default:
                return operator;
        }
    }

    private Predicate<Object> predicate(List<String> path) {
        if (peekKeyword("LIKE")) {
            return like(path);
        }
        if (peekKeyword("BETWEEN")) {
            return between(path);
        }
        if (peekKeyword("IN")) {
            return in(path);
        }
        if (peekKeyword("IS")) {
            return isNull(path);
        }
        Token operator = expectOneOf("a comparison operator", Type.EQUAL,
                Type.NOT_EQUAL, Type.LESS, Type.LESS_OR_EQUAL, Type.GREATER,
                Type.GREATER_OR_EQUAL);
        Token right = peek();
        if (right != null && right.type == Type.IDENTIFIER
                && !isKeyword(right.text, "TRUE")
                && !isKeyword(right.text, "FALSE")) {
            // Annex B: FIELDNAME RelOp FIELDNAME.
            return fieldComparison(path, operator.type, memberPath());
        }
        Object literal = literal();
        return comparison(path, operator.type, literal);
    }

    private Predicate<Object> fieldComparison(List<String> left, Type operator,
            List<String> right) {
        return sample -> {
            Object a = valueOf(sample, left);
            Object b = valueOf(sample, right);
            if (a == null || b == null) {
                return false;
            }
            return holds(operator, compare(a, b));
        };
    }

    private static boolean holds(Type operator, int order) {
        switch (operator) {
            case EQUAL:
                return order == 0;
            case NOT_EQUAL:
                return order != 0;
            case LESS:
                return order < 0;
            case LESS_OR_EQUAL:
                return order <= 0;
            case GREATER:
                return order > 0;
            default:
                return order >= 0;
        }
    }

    private Predicate<Object> like(List<String> path) {
        next();
        String pattern = expect(Type.STRING, "a string literal").text;
        String escape = "";
        if (peekKeyword("ESCAPE")) {
            next();
            escape = expect(Type.STRING, "a string literal").text;
        }
        java.util.regex.Pattern compiled =
                java.util.regex.Pattern.compile(likeToRegex(pattern, escape));
        return sample -> {
            Object value = valueOf(sample, path);
            return value != null
                    && compiled.matcher(String.valueOf(value)).matches();
        };
    }

    private Predicate<Object> between(List<String> path) {
        next();
        Object low = literal();
        expectKeyword("AND");
        Object high = literal();
        return sample -> {
            Object value = valueOf(sample, path);
            return value != null
                    && compare(value, low) >= 0
                    && compare(value, high) <= 0;
        };
    }

    private Predicate<Object> in(List<String> path) {
        next();
        expect(Type.LEFT_PAREN, "'('");
        List<Object> literals = new ArrayList<>();
        literals.add(literal());
        while (peekType(Type.COMMA)) {
            next();
            literals.add(literal());
        }
        expect(Type.RIGHT_PAREN, "')'");
        return sample -> {
            Object value = valueOf(sample, path);
            if (value == null) {
                return false;
            }
            for (Object candidate : literals) {
                if (compare(value, candidate) == 0) {
                    return true;
                }
            }
            return false;
        };
    }

    private Predicate<Object> isNull(List<String> path) {
        next();
        boolean negated = peekKeyword("NOT");
        if (negated) {
            next();
        }
        expectKeyword("NULL");
        return sample -> {
            boolean isNull = valueOf(sample, path) == null;
            return negated != isNull;
        };
    }

    private Predicate<Object> comparison(List<String> path, Type operator,
            Object literal) {
        return sample -> {
            Object value = valueOf(sample, path);
            if (value == null) {
                return false;
            }
            return holds(operator, compare(value, literal));
        };
    }

    /** A bare parameter index: the expression parameter must be true. */
    private Predicate<Object> parameterPredicate(Token token) {
        Object value = parameter(token.index);
        return sample -> value instanceof Boolean
                ? (Boolean) value
                : Boolean.parseBoolean(String.valueOf(value));
    }

    // --- Literals and parameters -------------------------------------------

    private Object literal() {
        Token token = peek();
        if (token == null) {
            throw error("a literal was expected");
        }
        switch (token.type) {
            case STRING:
                next();
                return token.text;
            case NUMBER:
                next();
                return new BigDecimal(token.text);
            case BOOLEAN:
                next();
                return isKeyword(token.text, "TRUE");
            case PARAMETER_INDEX:
                next();
                return parameter(token.index);
            case IDENTIFIER:
                // A bare word in operand position is a boolean literal.
                if (isKeyword(token.text, "TRUE")
                        || isKeyword(token.text, "FALSE")) {
                    next();
                    return isKeyword(token.text, "TRUE");
                }
                throw error("a literal was expected, got '" + token.text + "'");
            default:
                throw error("a literal was expected, got '" + token.text + "'");
        }
    }

    /**
     * The n-th expression parameter, read as a literal: a quoted text is a
     * string, {@code TRUE}/{@code FALSE} a boolean, anything else a number.
     */
    private Object parameter(int index) {
        if (index < 0 || index >= parameters.size()) {
            throw new IllegalArgumentException("the content filter '"
                    + expression + "' uses expression parameter " + index
                    + ", but " + parameters.size() + " were given");
        }
        String text = parameters.get(index);
        if (text == null) {
            return null;
        }
        String trimmed = text.trim();
        if (trimmed.length() >= 2
                && (trimmed.startsWith("'") && trimmed.endsWith("'")
                    || trimmed.startsWith("\"") && trimmed.endsWith("\""))) {
            return trimmed.substring(1, trimmed.length() - 1);
        }
        if (trimmed.equalsIgnoreCase("true") || trimmed.equalsIgnoreCase("false")) {
            return Boolean.valueOf(trimmed);
        }
        try {
            return new BigDecimal(trimmed);
        } catch (NumberFormatException notANumber) {
            return trimmed;
        }
    }

    // --- Comparing values of the sample ------------------------------------

    private static int compare(Object value, Object literal) {
        if (value instanceof Number && literal instanceof Number) {
            return decimal(value).compareTo(decimal(literal));
        }
        if (value instanceof Boolean && literal instanceof Boolean) {
            return Boolean.compare((Boolean) value, (Boolean) literal);
        }
        if (value instanceof Character || literal instanceof Character) {
            return String.valueOf(value).compareTo(String.valueOf(literal));
        }
        return String.valueOf(value).compareTo(String.valueOf(literal));
    }

    private static BigDecimal decimal(Object value) {
        if (value instanceof BigDecimal) {
            BigDecimal decimal = (BigDecimal) value;
            return decimal;
        }
        if (value instanceof Double || value instanceof Float) {
            return BigDecimal.valueOf(((Number) value).doubleValue());
        }
        return new BigDecimal(value.toString());
    }

    private static String likeToRegex(String pattern, String escape) {
        char escaper = escape.isEmpty() ? '\0' : escape.charAt(0);
        StringBuilder regex = new StringBuilder(pattern.length() * 2);
        for (int i = 0; i < pattern.length(); i++) {
            char c = pattern.charAt(i);
            if (escaper != '\0' && c == escaper && i + 1 < pattern.length()) {
                regex.append(java.util.regex.Pattern.quote(
                        String.valueOf(pattern.charAt(++i))));
            } else if (c == '%') {
                regex.append(".*");
            } else if (c == '_') {
                regex.append('.');
            } else {
                regex.append(java.util.regex.Pattern.quote(String.valueOf(c)));
            }
        }
        return regex.toString();
    }

    // --- Reading a member of a sample ---------------------------------------

    private static final Map<String, Member> MEMBERS = new ConcurrentHashMap<>();

    /** A resolved path of accessors, cached per class and path. */
    private static final class Member {
        private final List<AccessibleObject> steps;

        Member(List<AccessibleObject> steps) {
            this.steps = steps;
        }

        List<AccessibleObject> steps() {
            return steps;
        }

        Object get(Object sample) {
            Object current = sample;
            for (AccessibleObject step : steps) {
                if (current == null) {
                    return null;
                }
                if (step instanceof Method) {
                    Method method = (Method) step;
                    current = invoke(method, current);
                } else {
                    Field field = (Field) step;
                    try {
                        current = field.get(current);
                    } catch (IllegalAccessException e) {
                        throw new FilterException(
                                "a content filter cannot read '"
                                        + field.getName() + "' of the sample", e);
                    }
                }
            }
            return current;
        }
    }

    /** A content filter that cannot be evaluated against a sample. */
    public static final class FilterException extends RuntimeException {

        private static final long serialVersionUID = 1L;

        FilterException(String message, Throwable cause) {
            super(message, cause);
        }
    }

    private static Object valueOf(Object sample, List<String> path) {
        if (path.isEmpty()) {
            return sample;
        }
        if (sample == null) {
            // A sample without data (a dispose, an unregister) has no members.
            return null;
        }
        if (sample instanceof Map) {
            // A row of named values (the joined row of a MultiTopic): the
            // first name selects the entry, the rest navigates into it.
            Map<?, ?> row = (Map<?, ?>) sample;
            Object head = row.get(path.get(0));
            if (head == null && !row.containsKey(path.get(0))) {
                throw new IllegalArgumentException("the content filter reads '"
                        + String.join(".", path) + "', but the row has no member '"
                        + path.get(0) + "'");
            }
            return valueOf(head, path.subList(1, path.size()));
        }
        String key = sample.getClass().getName() + "#" + String.join(".", path);
        Member member = MEMBERS.computeIfAbsent(key,
                ignored -> resolve(sample.getClass(), path));
        return member.get(sample);
    }

    private static Member resolve(Class<?> type, List<String> path) {
        List<AccessibleObject> steps = new ArrayList<>();
        Class<?> current = type;
        for (String name : path) {
            AccessibleObject step = accessor(current, name);
            if (step == null) {
                throw new IllegalArgumentException("the content filter reads '"
                        + String.join(".", path) + "', but " + type.getName()
                        + " has no member '" + name + "'");
            }
            steps.add(step);
            if (step instanceof Method) {
                Method method = (Method) step;
                current = method.getReturnType();
            } else {
                current = ((Field) step).getType();
            }
        }
        return new Member(steps);
    }

    private static AccessibleObject accessor(Class<?> type, String name) {
        String capitalized = Character.toUpperCase(name.charAt(0))
                + name.substring(1);
        for (String candidate : new String[] {name, "get" + capitalized,
                "is" + capitalized}) {
            try {
                Method method = type.getMethod(candidate);
                if (method.getParameterCount() == 0
                        && method.getReturnType() != void.class) {
                    return method;
                }
            } catch (NoSuchMethodException absent) {
                // try the next spelling
            }
        }
        try {
            return type.getField(name);
        } catch (NoSuchFieldException absent) {
            return null;
        }
    }

    private static Object invoke(Method method, Object target) {
        try {
            return method.invoke(target);
        } catch (IllegalAccessException | InvocationTargetException e) {
            throw new FilterException(
                    "a content filter cannot read '" + method.getName()
                            + "' of the sample", e);
        }
    }

    // --- The tokenizer ------------------------------------------------------

    private enum Type {
        IDENTIFIER, NUMBER, STRING, BOOLEAN, PARAMETER_INDEX,
        EQUAL, NOT_EQUAL, LESS, LESS_OR_EQUAL, GREATER, GREATER_OR_EQUAL,
        LEFT_PAREN, RIGHT_PAREN, COMMA, DOT, PERCENT, END
    }

    private static final class Token {
        private final Type type;
        private final String text;
        private final int index;

        Token(Type type, String text, int index) {
            this.type = type;
            this.text = text;
            this.index = index;
        }

        Type type() {
            return type;
        }

        String text() {
            return text;
        }

        int index() {
            return index;
        }

        @Override
        public String toString() {
            return type == Type.PARAMETER_INDEX
                    ? "%" + index
                    : type.name().toLowerCase() + "(" + text + ")";
        }
    }

    private static List<Token> tokenize(String expression) {
        List<Token> tokens = new ArrayList<>();
        int i = 0;
        while (i < expression.length()) {
            char c = expression.charAt(i);
            if (Character.isWhitespace(c)) {
                i++;
                continue;
            }
            switch (c) {
                case '(':
                    tokens.add(new Token(Type.LEFT_PAREN, "(", -1));
                    i++;
                    continue;
                case ')':
                    tokens.add(new Token(Type.RIGHT_PAREN, ")", -1));
                    i++;
                    continue;
                case ',':
                    tokens.add(new Token(Type.COMMA, ",", -1));
                    i++;
                    continue;
                case '.':
                    tokens.add(new Token(Type.DOT, ".", -1));
                    i++;
                    continue;
                case '%': {
                    i++;
                    Token parameter = parameterIndex(expression, i);
                    tokens.add(parameter);
                    i += parameter.text().length();
                    continue;
                }
                case '=':
                    if (i + 1 < expression.length()
                            && expression.charAt(i + 1) == '=') {
                        tokens.add(new Token(Type.EQUAL, "==", -1));
                        i += 2;
                    } else {
                        tokens.add(new Token(Type.EQUAL, "=", -1));
                        i++;
                    }
                    continue;
                case '!':
                case '<':
                case '>': {
                    String two = i + 1 < expression.length()
                            ? expression.substring(i, i + 2) : "";
                    if (two.equals("!=") || two.equals("<>")) {
                        tokens.add(new Token(Type.NOT_EQUAL, two, -1));
                        i += 2;
                    } else if (two.equals("<=") || two.equals(">=")) {
                        tokens.add(new Token(
                                two.charAt(0) == '<' ? Type.LESS_OR_EQUAL
                                        : Type.GREATER_OR_EQUAL, two, -1));
                        i += 2;
                    } else {
                        tokens.add(new Token(
                                c == '!' ? Type.NOT_EQUAL
                                        : c == '<' ? Type.LESS
                                                : Type.GREATER,
                                String.valueOf(c), -1));
                        i++;
                    }
                    continue;
                }
                case '\'':
                case '"': {
                    int end = expression.indexOf(c, i + 1);
                    if (end < 0) {
                        throw new IllegalArgumentException("the content filter '"
                                + expression + "' has an unterminated string");
                    }
                    tokens.add(new Token(Type.STRING,
                            expression.substring(i + 1, end), -1));
                    i = end + 1;
                    continue;
                }
                default:
                    break;
            }
            if (Character.isDigit(c) || (c == '-' && i + 1 < expression.length()
                    && Character.isDigit(expression.charAt(i + 1)))) {
                int start = i;
                if (c == '-') {
                    i++;
                }
                while (i < expression.length()
                        && (Character.isDigit(expression.charAt(i))
                            || expression.charAt(i) == '.')) {
                    i++;
                }
                tokens.add(new Token(Type.NUMBER,
                        expression.substring(start, i), -1));
                continue;
            }
            if (Character.isJavaIdentifierStart(c)) {
                int start = i;
                while (i < expression.length()
                        && Character.isJavaIdentifierPart(expression.charAt(i))) {
                    i++;
                }
                String text = expression.substring(start, i);
                Type type = isKeyword(text, "TRUE") || isKeyword(text, "FALSE")
                        ? Type.BOOLEAN
                        : Type.IDENTIFIER;
                tokens.add(new Token(type, text, -1));
                continue;
            }
            throw new IllegalArgumentException("the content filter '"
                    + expression + "' has an unexpected character '" + c + "'");
        }
        return tokens;
    }

    private static Token parameterIndex(String expression, int at) {
        int start = at;
        while (at < expression.length()
                && Character.isDigit(expression.charAt(at))) {
            at++;
        }
        if (start == at) {
            throw new IllegalArgumentException("the content filter '"
                    + expression + "' has a '%' without a parameter index");
        }
        return new Token(Type.PARAMETER_INDEX, expression.substring(start, at),
                Integer.parseInt(expression.substring(start, at)));
    }

    private static boolean isKeyword(String text, String keyword) {
        return text.equalsIgnoreCase(keyword);
    }

    // --- The token stream ---------------------------------------------------

    private Token peek() {
        return at < tokens.size() ? tokens.get(at) : null;
    }

    private Token next() {
        if (at >= tokens.size()) {
            throw error("the expression ends unexpectedly");
        }
        return tokens.get(at++);
    }

    private Token expect(Type type, String what) {
        Token token = peek();
        if (token == null || token.type != type) {
            throw error(what + " was expected");
        }
        return next();
    }

    private Token expectOneOf(String what, Type... allowed) {
        Token token = peek();
        if (token == null) {
            throw error(what + " was expected");
        }
        for (Type type : allowed) {
            if (token.type == type) {
                return next();
            }
        }
        throw error(what + " was expected, got '" + token.text + "'");
    }

    private boolean peekType(Type type) {
        Token token = peek();
        return token != null && token.type == type;
    }

    private boolean peekKeyword(String keyword) {
        Token token = peek();
        return token != null && token.type == Type.IDENTIFIER
                && isKeyword(token.text, keyword);
    }

    private void expectKeyword(String keyword) {
        if (!peekKeyword(keyword)) {
            throw error("'" + keyword + "' was expected");
        }
        next();
    }

    private void expectEnd() {
        if (at < tokens.size()) {
            throw error("'" + tokens.get(at).text + "' is not expected here");
        }
    }

    private IllegalArgumentException error(String what) {
        return new IllegalArgumentException(
                what + " in the content filter '" + expression + "'");
    }
}
