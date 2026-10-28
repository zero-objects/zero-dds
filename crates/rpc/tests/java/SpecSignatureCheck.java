// SPDX-License-Identifier: Apache-2.0
import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;

/**
 * DDS-RPC 1.0 §7.11.2: "The machine readable files associated with this
 * specification represent the normative language bindings for Java."
 *
 * <p>Checks the runtime against every public declaration of those files, as
 * {@code extract_spec_signatures.py} wrote them to
 * {@code omg-dds-rpc-java.signatures}: each type with its kind, the number of
 * its type parameters and its supertypes; each method with its name, whether it
 * is static, the number of its type parameters, its parameter types and its
 * return type after erasure; each constructor; each enum constant. A method's
 * return type may be a subtype of the one in the files (what a caller compiled
 * against the files assigns it to still holds it) but not the same type,
 * which code compiled against the files does not find (the descriptor of a
 * method names its return type); it is listed as {@code NARROWER} and counts
 * as a deviation, as anything else does. What the runtime declares beyond the
 * files is not checked.
 *
 * <p>Prints every deviation and the number of declarations checked, and exits
 * with 1 when there is a deviation.
 */
public final class SpecSignatureCheck {

    private SpecSignatureCheck() {}

    /** The methods whose return type is a subtype of the one in the files, as text lines. */
    private static final List<String> NARROWER = new ArrayList<>();

    public static void main(String[] args) throws Exception {
        List<String> deviations = new ArrayList<>();
        int checked = 0;
        for (String line : Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8)) {
            if (line.isEmpty() || line.startsWith("#")) {
                continue;
            }
            ++checked;
            String[] f = line.split(" ");
            String problem;
            try {
                switch (f[0]) {
                    case "type":
                        problem = checkType(f);
                        break;
                    case "method":
                        problem = checkMethod(f);
                        break;
                    case "ctor":
                        problem = checkConstructor(f);
                        break;
                    case "constant":
                        problem = checkConstant(f);
                        break;
                    default:
                        problem = "an entry the check does not know";
                }
            } catch (ClassNotFoundException missing) {
                problem = "no class " + missing.getMessage();
            }
            if (problem != null) {
                deviations.add(line + "\n    " + problem);
            }
        }
        for (String deviation : deviations) {
            System.out.println("DEVIATION " + deviation);
        }
        for (String narrower : NARROWER) {
            System.out.println("NARROWER " + narrower);
        }
        System.out.println(checked + " declarations of omg-dds-rpc-java.zip checked, "
                + deviations.size() + " deviations, " + NARROWER.size()
                + " return types narrower than the files' ones");
        if (!deviations.isEmpty() || !NARROWER.isEmpty()) {
            System.exit(1);
        }
        System.out.println("every signature held");
    }

    private static Class<?> load(String name) throws ClassNotFoundException {
        switch (name) {
            case "void":
                return void.class;
            case "boolean":
                return boolean.class;
            case "byte":
                return byte.class;
            case "short":
                return short.class;
            case "int":
                return int.class;
            case "long":
                return long.class;
            case "char":
                return char.class;
            case "float":
                return float.class;
            case "double":
                return double.class;
            default:
                break;
        }
        if (name.endsWith("[]")) {
            return java.lang.reflect.Array.newInstance(load(name.substring(0, name.length() - 2)), 0)
                    .getClass();
        }
        return Class.forName(name, false, SpecSignatureCheck.class.getClassLoader());
    }

    private static Class<?>[] parameters(String list) throws ClassNotFoundException {
        String inner = list.substring(1, list.length() - 1);
        if (inner.isEmpty()) {
            return new Class<?>[0];
        }
        String[] names = inner.split(",");
        Class<?>[] out = new Class<?>[names.length];
        for (int i = 0; i < names.length; ++i) {
            out[i] = load(names[i]);
        }
        return out;
    }

    /** {@code type <fqn> <kind> <type-parameters> <supertype>...} */
    private static String checkType(String[] f) throws ClassNotFoundException {
        Class<?> type = load(f[1]);
        int mods = type.getModifiers();
        if (!Modifier.isPublic(mods)) {
            return "not public";
        }
        String kind;
        if (type.isEnum()) {
            kind = "enum";
        } else if (type.isInterface()) {
            kind = "interface";
        } else if (Modifier.isAbstract(mods)) {
            kind = "abstract-class";
        } else if (Modifier.isFinal(mods)) {
            kind = "final-class";
        } else {
            kind = "class";
        }
        if (!kind.equals(f[2])) {
            return "is " + kind;
        }
        int parameters = type.getTypeParameters().length;
        if (parameters != Integer.parseInt(f[3])) {
            return "has " + parameters + " type parameters";
        }
        for (int i = 4; i < f.length; ++i) {
            if (!load(f[i]).isAssignableFrom(type)) {
                return "is not a " + f[i];
            }
        }
        return null;
    }

    /** {@code method <fqn> <static|instance> <type-parameters> <name> <return> (<parameter>,...)} */
    private static String checkMethod(String[] f) throws ClassNotFoundException {
        Class<?> type = load(f[1]);
        Class<?>[] params = parameters(f[6]);
        Method found;
        try {
            found = type.getMethod(f[4], params);
        } catch (NoSuchMethodException none) {
            return "no public method with these parameter types";
        }
        boolean isStatic = Modifier.isStatic(found.getModifiers());
        if (isStatic != f[2].equals("static")) {
            return isStatic ? "is static" : "is not static";
        }
        int typeParameters = found.getTypeParameters().length;
        if (typeParameters != Integer.parseInt(f[3])) {
            return "has " + typeParameters + " type parameters";
        }
        Class<?> expected = load(f[5]);
        Class<?> actual = found.getReturnType();
        if (actual != expected && (expected.isPrimitive() || actual.isPrimitive()
                || !expected.isAssignableFrom(actual))) {
            return "returns " + actual.getName();
        }
        if (actual != expected) {
            NARROWER.add(f[1] + "." + f[4] + f[6] + " returns " + actual.getName()
                    + " (the files: " + expected.getName() + ")");
        }
        return null;
    }

    /** {@code ctor <fqn> (<parameter>,...)} */
    private static String checkConstructor(String[] f) throws ClassNotFoundException {
        Class<?> type = load(f[1]);
        try {
            Constructor<?> found = type.getDeclaredConstructor(parameters(f[2]));
            int mods = found.getModifiers();
            // an abstract class's constructor is called by its subclasses
            boolean reachable = Modifier.isPublic(mods)
                    || (Modifier.isAbstract(type.getModifiers()) && Modifier.isProtected(mods));
            return reachable ? null : "the constructor is not public";
        } catch (NoSuchMethodException none) {
            return "no constructor with these parameter types";
        }
    }

    /** {@code constant <fqn> <name>} */
    private static String checkConstant(String[] f) throws ClassNotFoundException {
        Class<?> type = load(f[1]);
        for (Object constant : type.getEnumConstants()) {
            if (((Enum<?>) constant).name().equals(f[2])) {
                return null;
            }
        }
        return "no constant " + f[2];
    }
}
