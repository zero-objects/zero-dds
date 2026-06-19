// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! REP-2008 Type-Mapping `.msg`/`.srv` → IDL.
//!
//! Spec: `zerodds-ros2-bridge-1.0.md` §5.2 (= REP-2008).
//!
//! ROS-2 `.msg`-Files definieren Strukturen wie:
//!
//! ```text
//! # Comment
//! string name
//! int32 age
//! bool active
//! float64[3] position
//! geometry_msgs/Point center
//! ```
//!
//! Mapping-Regeln (REP-2008 Tab 1):
//!
//! | ROS-Typ      | IDL-Typ     |
//! |-------------|-------------|
//! | bool        | boolean     |
//! | int8        | octet       |
//! | uint8       | octet       |
//! | int16       | short       |
//! | uint16      | unsigned short |
//! | int32       | long        |
//! | uint32      | unsigned long |
//! | int64       | long long   |
//! | uint64      | unsigned long long |
//! | float32     | float       |
//! | float64     | double      |
//! | string      | string      |
//! | T[]         | sequence<T> |
//! | T[N]        | T[N]        |
//!
//! This implementation parses `.msg` source, builds a
//! `MsgStruct` AST and can render it as IDL source.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Parsed ROS-2 field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgField {
    /// Field name (snake_case).
    pub name: String,
    /// ROS type identifier (e.g. `string`, `int32`, `geometry_msgs/Point`).
    pub ros_type: String,
    /// Array spec: `None` = scalar, `Some(None)` = unbounded sequence,
    /// `Some(Some(N))` = fixed-size N.
    pub array: Option<Option<usize>>,
}

/// Parsed ROS-2 struct (`.msg` parsed into structured form).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgStruct {
    /// `<package>/<Type>` (e.g. `geometry_msgs/Point`).
    pub fully_qualified: String,
    /// Fields.
    pub fields: Vec<MsgField>,
}

/// Parser error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Line does not match `<type>[<arr>] <name>`.
    BadLine(String),
    /// Array spec is not parsable.
    BadArraySpec(String),
}

/// Parse `.msg` source code.
///
/// # Errors
/// `BadLine`/`BadArraySpec` on syntax errors.
pub fn parse_msg(fully_qualified: &str, source: &str) -> Result<MsgStruct, ParseError> {
    let mut fields = Vec::new();
    for raw in source.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        // Constants (starting with `<type> <name>=<value>`) are
        // allowed in ROS-2 — we ignore them for the mapping. Detected
        // by the `=` to the right of the name.
        if line.contains('=') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let typ = parts
            .next()
            .ok_or_else(|| ParseError::BadLine(line.into()))?;
        let name = parts
            .next()
            .ok_or_else(|| ParseError::BadLine(line.into()))?;
        let (ros_type, array) = parse_type(typ)?;
        fields.push(MsgField {
            name: name.into(),
            ros_type,
            array,
        });
    }
    Ok(MsgStruct {
        fully_qualified: fully_qualified.into(),
        fields,
    })
}

fn parse_type(typ: &str) -> Result<(String, Option<Option<usize>>), ParseError> {
    if let Some(idx) = typ.find('[') {
        let base = &typ[..idx];
        let arr = &typ[idx..];
        if !arr.ends_with(']') {
            return Err(ParseError::BadArraySpec(typ.into()));
        }
        let inner = &arr[1..arr.len() - 1];
        if inner.is_empty() {
            // unbounded
            Ok((base.into(), Some(None)))
        } else {
            let n: usize = inner
                .parse()
                .map_err(|_| ParseError::BadArraySpec(typ.into()))?;
            Ok((base.into(), Some(Some(n))))
        }
    } else {
        Ok((typ.into(), None))
    }
}

/// Mapping ROS-Typ → IDL-Typ. REP-2008 Tab 1.
#[must_use]
pub fn ros_to_idl_type(ros: &str) -> String {
    match ros {
        "bool" => "boolean".into(),
        "byte" | "int8" | "uint8" => "octet".into(),
        "char" => "char".into(),
        "int16" => "short".into(),
        "uint16" => "unsigned short".into(),
        "int32" => "long".into(),
        "uint32" => "unsigned long".into(),
        "int64" => "long long".into(),
        "uint64" => "unsigned long long".into(),
        "float32" => "float".into(),
        "float64" => "double".into(),
        "string" => "string".into(),
        // `package/Type` → `package::Type`.
        other if other.contains('/') => other.replace('/', "::"),
        other => other.to_string(),
    }
}

/// Render a `MsgStruct` as IDL source.
#[must_use]
pub fn render_idl(s: &MsgStruct) -> String {
    let mut out = String::new();
    let (mod_path, type_name) = split_pkg(&s.fully_qualified);
    if !mod_path.is_empty() {
        out.push_str(&format!("module {mod_path} {{\n"));
    }
    out.push_str(&format!("  struct {type_name} {{\n"));
    for f in &s.fields {
        let idl_t = ros_to_idl_type(&f.ros_type);
        let line = match f.array {
            None => format!("    {idl_t} {};\n", f.name),
            Some(None) => format!("    sequence<{idl_t}> {};\n", f.name),
            Some(Some(n)) => format!("    {idl_t} {}[{n}];\n", f.name),
        };
        out.push_str(&line);
    }
    out.push_str("  };\n");
    if !mod_path.is_empty() {
        out.push_str("};\n");
    }
    out
}

fn split_pkg(fq: &str) -> (String, String) {
    if let Some(idx) = fq.find('/') {
        (fq[..idx].into(), fq[idx + 1..].into())
    } else {
        (String::new(), fq.into())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_msg_three_fields() {
        let src = "string name\nint32 age\nbool active\n";
        let s = parse_msg("test/Person", src).expect("parse");
        assert_eq!(s.fields.len(), 3);
        assert_eq!(s.fields[0].ros_type, "string");
        assert_eq!(s.fields[0].name, "name");
        assert_eq!(s.fields[1].ros_type, "int32");
        assert_eq!(s.fields[2].ros_type, "bool");
    }

    #[test]
    fn parse_skips_comments_and_blank_lines() {
        let src = "# top comment\n\nstring x  # trailing comment\n";
        let s = parse_msg("X/Y", src).expect("parse");
        assert_eq!(s.fields.len(), 1);
        assert_eq!(s.fields[0].name, "x");
    }

    #[test]
    fn parse_array_spec() {
        let src = "float64[3] position\nint32[] dynamic\n";
        let s = parse_msg("X/Y", src).expect("parse");
        assert_eq!(s.fields[0].array, Some(Some(3)));
        assert_eq!(s.fields[1].array, Some(None));
    }

    #[test]
    fn parse_constants_are_ignored() {
        let src = "uint32 PI=3\nstring name\n";
        let s = parse_msg("X/Y", src).expect("parse");
        // Constant lines (with `=`) skipped → 1 field "name".
        assert_eq!(s.fields.len(), 1);
        assert_eq!(s.fields[0].name, "name");
    }

    #[test]
    fn idl_mapping_primitives() {
        assert_eq!(ros_to_idl_type("bool"), "boolean");
        assert_eq!(ros_to_idl_type("int32"), "long");
        assert_eq!(ros_to_idl_type("uint32"), "unsigned long");
        assert_eq!(ros_to_idl_type("float64"), "double");
        assert_eq!(ros_to_idl_type("string"), "string");
    }

    #[test]
    fn idl_mapping_qualified_type() {
        assert_eq!(
            ros_to_idl_type("geometry_msgs/Point"),
            "geometry_msgs::Point"
        );
    }

    #[test]
    fn render_idl_produces_module_and_struct() {
        let s = parse_msg(
            "test/Person",
            "string name\nint32 age\nfloat64[3] position\n",
        )
        .expect("parse");
        let idl = render_idl(&s);
        assert!(idl.contains("module test {"));
        assert!(idl.contains("struct Person"));
        assert!(idl.contains("string name;"));
        assert!(idl.contains("long age;"));
        assert!(idl.contains("double position[3];"));
    }
}
