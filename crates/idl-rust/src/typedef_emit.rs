// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Emittiert IDL `typedef` → Rust `pub type X = Y;`.

use zerodds_idl::ast::types::{Declarator, TypedefDecl};

use crate::error::{Result, RustGenError};
use crate::type_map::escape_keyword;
use crate::type_map::{const_expr_as_usize, rust_type_for};

/// Emits one or more `pub type Alias = ...;` statements.
pub fn emit_typedef(out: &mut String, td: &TypedefDecl) -> Result<()> {
    let base_type = rust_type_for(&td.type_spec)?;
    for declarator in &td.declarators {
        match declarator {
            Declarator::Simple(name) => {
                out.push_str(&format!(
                    "pub type {} = {};\n",
                    escape_keyword(&name.text),
                    base_type
                ));
            }
            Declarator::Array(arr) => {
                let mut wrapped = base_type.clone();
                for size_expr in arr.sizes.iter().rev() {
                    let size =
                        const_expr_as_usize(size_expr).ok_or(RustGenError::InvalidAnnotation {
                            name: "typedef-array-size".to_string(),
                            reason: "non-integer dimension",
                        })?;
                    wrapped = format!("[{wrapped}; {size}]");
                }
                out.push_str(&format!(
                    "pub type {} = {};\n",
                    escape_keyword(&arr.name.text),
                    wrapped
                ));
            }
        }
    }
    Ok(())
}
