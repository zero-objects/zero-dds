// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Names: the two naming schemes of IDL4 to Java Language Mapping 1.0
//! §7.1.1, the four transformations of §7.1.1.2.1 to §7.1.1.2.4, the
//! reserved names of §7.1.2 and the suffix rule of §7.1.1.3.
//!
//! The transformations follow the rules of their clauses. Where an
//! example in the specification disagrees with its own rule —
//! `longlong_variable` printed as `longLongVariable` in §7.2.4.3.1, which
//! the camel-case rule makes `longlongVariable`, and the parameter `URL`
//! printed unchanged, which the rule makes `uRL` — the rule is what is
//! implemented.

/// The naming scheme in force, §7.1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Naming {
    /// §7.1.1.1: names as they are in IDL.
    #[default]
    Idl,
    /// §7.1.1.2: names after the JavaBeans 1.01 conventions, Table 8.1.
    Java,
}

impl Naming {
    /// The name the command line and `@java_mapping` give the scheme.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idl => "idl",
            Self::Java => "java",
        }
    }

    /// The scheme a command-line name or an `apply_naming_convention`
    /// value selects.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "idl" | "IDL_NAMING_CONVENTION" => Some(Self::Idl),
            "java" | "JAVA_NAMING_CONVENTION" => Some(Self::Java),
            _ => None,
        }
    }
}

/// §7.1.1.2.1: "The first letter after each underscore shall be
/// capitalized and all underscores shall be removed. The first letter of
/// the IDL name shall be capitalized."
#[must_use]
pub fn pascal_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut capitalise = true;
    for ch in name.chars() {
        if ch == '_' {
            capitalise = true;
            continue;
        }
        if capitalise {
            out.extend(ch.to_uppercase());
            capitalise = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// §7.1.1.2.2: "The first letter after each underscore shall be
/// capitalized and all underscores shall be removed. The first letter of
/// the IDL name shall be lower case."
#[must_use]
pub fn camel_case(name: &str) -> String {
    let pascal = pascal_case(name);
    let mut chars = pascal.chars();
    match chars.next() {
        Some(first) => {
            let mut out: String = first.to_lowercase().collect();
            out.push_str(chars.as_str());
            out
        }
        None => pascal,
    }
}

/// §7.1.1.2.3: "Every letter shall be capitalized. All underscores shall
/// remain unchanged."
#[must_use]
pub fn all_uppercase(name: &str) -> String {
    name.chars().flat_map(char::to_uppercase).collect()
}

/// §7.1.1.2.4: "Every letter shall be lowercase. All underscores shall
/// remain unchanged."
#[must_use]
pub fn all_lowercase(name: &str) -> String {
    name.chars().flat_map(char::to_lowercase).collect()
}

/// The keywords of the Java language, §7.1.2 (J2SE 8.0 clause 3.9), as
/// the clause lists them.
pub const JAVA_KEYWORDS: [&str; 50] = [
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
];

/// "The additional Java constants/literals", §7.1.2.
pub const JAVA_LITERALS: [&str; 3] = ["true", "false", "null"];

/// The methods of `java.lang.Object` a mapped method name may collide
/// with, §7.1.2 (J2SE 8.0 clause 4.3.2).
pub const OBJECT_METHODS: [&str; 9] = [
    "clone",
    "notifyAll",
    "getClass",
    "notify",
    "finalize",
    "wait",
    "equals",
    "toString",
    "hashCode",
];

/// The suffix of the class a value type's state and operations go to,
/// §7.6, and the reserved suffix of §7.1.1.3.
pub const ABSTRACT_SUFFIX: &str = "Abstract";

/// The class of the alternative mapping of constants, §7.2.3.1, which
/// §7.1.2 reserves in every package.
pub const CONSTANTS_CLASS: &str = "Constants";

/// A Java keyword or literal.
#[must_use]
pub fn is_reserved_word(name: &str) -> bool {
    JAVA_KEYWORDS.contains(&name) || JAVA_LITERALS.contains(&name)
}

/// §7.1.1: "if a mapped name or identifier collides with one of the names
/// reserved in Clause 7.1.2, the collision shall be resolved by
/// prepending an underscore". For a field, a parameter, an enumerator and
/// a package segment the reserved names are the keywords and literals.
#[must_use]
pub fn escape(mapped: String) -> String {
    if is_reserved_word(&mapped) {
        format!("_{mapped}")
    } else {
        mapped
    }
}

/// The name of a mapped type: the keywords and literals, `Constants`
/// (§7.1.2) and a name ending in `Abstract` (§7.1.1.3: "an IDL struct
/// whose name is FooAbstract shall be mapped to _FooAbstract") take the
/// underscore.
#[must_use]
pub fn escape_type(mapped: String) -> String {
    if is_reserved_word(&mapped)
        || mapped == CONSTANTS_CLASS
        || (mapped.ends_with(ABSTRACT_SUFFIX) && mapped.len() > ABSTRACT_SUFFIX.len())
    {
        format!("_{mapped}")
    } else {
        mapped
    }
}

/// The name of a mapped method: a keyword, a literal or a method of
/// `java.lang.Object` takes the underscore (§7.1.2: "treated as reserved
/// if used in a context where the mapping collides with the following
/// methods on java.lang.Object").
#[must_use]
pub fn escape_method(mapped: String) -> String {
    if is_reserved_word(&mapped) || OBJECT_METHODS.contains(&mapped.as_str()) {
        format!("_{mapped}")
    } else {
        mapped
    }
}

/// The names one IDL name takes under a scheme, Table 8.1.
impl Naming {
    /// A module: the package segment.
    #[must_use]
    pub fn package(self, name: &str) -> String {
        escape(match self {
            Self::Idl => name.to_string(),
            Self::Java => all_lowercase(name),
        })
    }

    /// A structure, union, enumeration, interface, exception, bit set,
    /// bit mask, value type or annotation: the class name.
    #[must_use]
    pub fn type_name(self, name: &str) -> String {
        escape_type(match self {
            Self::Idl => name.to_string(),
            Self::Java => pascal_case(name),
        })
    }

    /// A constant's own class under the default mapping, §7.2.3 (the
    /// clause's example: `e` is `E`, `my_string` is `MyString`).
    #[must_use]
    pub fn constant_class(self, name: &str) -> String {
        self.type_name(name)
    }

    /// A constant's field in the alternative mapping, §7.2.3.1 (Table
    /// 8.1: All Uppercase).
    #[must_use]
    pub fn constant_field(self, name: &str) -> String {
        escape(match self {
            Self::Idl => name.to_string(),
            Self::Java => all_uppercase(name),
        })
    }

    /// An enumerator (Table 8.1: All Uppercase), and a flag of a bit mask
    /// (§7.14.3.3's example: `FLAG0`).
    #[must_use]
    pub fn enumerator(self, name: &str) -> String {
        escape(match self {
            Self::Idl => name.to_string(),
            Self::Java => all_uppercase(name),
        })
    }

    /// The private field that holds a member, and the parameter of its
    /// modifier (Table 8.1: Camel Case).
    #[must_use]
    pub fn variable(self, name: &str) -> String {
        escape(match self {
            Self::Idl => name.to_string(),
            Self::Java => camel_case(name),
        })
    }

    /// An operation (Table 8.1: Camel Case).
    #[must_use]
    pub fn method(self, name: &str) -> String {
        escape_method(match self {
            Self::Idl => name.to_string(),
            Self::Java => camel_case(name),
        })
    }

    /// The accessor of a member, attribute or bit field: `get_<name>`
    /// under the IDL scheme, `get<PascalName>` under the Java one.
    #[must_use]
    pub fn getter(self, name: &str) -> String {
        escape_method(match self {
            Self::Idl => format!("get_{name}"),
            Self::Java => format!("get{}", pascal_case(name)),
        })
    }

    /// The modifier of a member, attribute or bit field.
    #[must_use]
    pub fn setter(self, name: &str) -> String {
        escape_method(match self {
            Self::Idl => format!("set_{name}"),
            Self::Java => format!("set{}", pascal_case(name)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every example §7.1.1.2.1 gives.
    #[test]
    fn pascal_case_is_the_clause_examples() {
        for (idl, java) in [
            ("pascalcase", "Pascalcase"),
            ("PASCALCASE", "PASCALCASE"),
            ("Pascal_Case", "PascalCase"),
            ("pascal_case", "PascalCase"),
            ("Pascal_case", "PascalCase"),
            ("PASCAL_case", "PASCALCase"),
            ("PASCAL_CASE", "PASCALCASE"),
            ("_pascalCase", "PascalCase"),
            ("_PascalCase", "PascalCase"),
            ("pascal_case_", "PascalCase"),
            ("pascalCase", "PascalCase"),
            ("PascalCase", "PascalCase"),
            ("PASCALcase", "PASCALcase"),
            ("PASCALCase", "PASCALCase"),
        ] {
            assert_eq!(pascal_case(idl), java, "{idl}");
        }
    }

    /// Every example §7.1.1.2.2 gives.
    #[test]
    fn camel_case_is_the_clause_examples() {
        for (idl, java) in [
            ("camelcase", "camelcase"),
            ("CAMELCASE", "cAMELCASE"),
            ("Camel_Case", "camelCase"),
            ("camel_case", "camelCase"),
            ("Camel_case", "camelCase"),
            ("camel_Case", "camelCase"),
            ("CAMEL_case", "cAMELCase"),
            ("CAMEL_CASE", "cAMELCASE"),
            ("_camelCase", "camelCase"),
            ("_CamelCase", "camelCase"),
            ("camel_case_", "camelCase"),
            ("camelCase", "camelCase"),
            ("CamelCase", "camelCase"),
            ("CAMELcase", "cAMELcase"),
            ("CAMELCase", "cAMELCase"),
        ] {
            assert_eq!(camel_case(idl), java, "{idl}");
        }
    }

    /// Every example §7.1.1.2.3 and §7.1.1.2.4 give.
    #[test]
    fn upper_and_lower_case_are_the_clause_examples() {
        for (idl, java) in [
            ("ALL", "ALL"),
            ("ALL_UPPERCASE", "ALL_UPPERCASE"),
            ("all", "ALL"),
            ("all_uppercase", "ALL_UPPERCASE"),
            ("allUppercase", "ALLUPPERCASE"),
            ("AllUppercase", "ALLUPPERCASE"),
            ("ALLUppercase", "ALLUPPERCASE"),
        ] {
            assert_eq!(all_uppercase(idl), java, "{idl}");
        }
        for (idl, java) in [
            ("ALL", "all"),
            ("ALL_LOWERCASE", "all_lowercase"),
            ("all", "all"),
            ("all_lowercase", "all_lowercase"),
            ("allLowercase", "alllowercase"),
            ("AllLowercase", "alllowercase"),
            ("ALLLowercase", "alllowercase"),
        ] {
            assert_eq!(all_lowercase(idl), java, "{idl}");
        }
    }

    #[test]
    fn reserved_names_take_a_leading_underscore() {
        assert_eq!(Naming::Idl.type_name("class"), "_class");
        assert_eq!(Naming::Idl.type_name("FooAbstract"), "_FooAbstract");
        assert_eq!(Naming::Idl.type_name("Constants"), "_Constants");
        assert_eq!(Naming::Idl.type_name("Abstract"), "Abstract");
        assert_eq!(Naming::Java.type_name("class"), "Class");
        assert_eq!(Naming::Idl.variable("null"), "_null");
        assert_eq!(Naming::Java.getter("class"), "_getClass");
        assert_eq!(Naming::Idl.method("hashCode"), "_hashCode");
        assert_eq!(Naming::Java.package("MY_MATH"), "my_math");
        assert_eq!(Naming::Idl.package("package"), "_package");
    }
}
