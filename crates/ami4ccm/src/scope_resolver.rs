// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The symbol table the AMI4CCM naming resolver checks against —
//! Spec §7.5 / §7.3.1.
//!
//! §7.5 requires the generated names to be unique "in the interface or
//! any of the interface's base interfaces" and within the compilation
//! scope. The first two the transformation reads off the interface
//! itself; this is the third.
//!
//! The old walker descended a syntax tree and matched on twenty
//! declaration forms to arrive at the names in it, and had to be
//! extended whenever a form was added. The model already holds that
//! table: every declaration that has a scoped name is in
//! [`zerodds_ast::Ast::all_nodes`], and there is nothing to walk.

use alloc::format;
use alloc::string::ToString;

use zerodds_ast::Ast;

use crate::transform::TransformContext;

/// Extend a context with every declaration in a model.
///
/// Each name is registered in both forms: bare (`Order`) and qualified
/// (`Trading::Order`). The resolver matches the bare form, because the
/// AMI4CCM names are emitted without a module prefix; the qualified form
/// is there for callers that work with full names.
pub fn populate_from_model(ctx: &mut TransformContext, ast: &Ast) {
    for any in ast.all_nodes() {
        let Some(name) = ast.scoped_name_of(any) else {
            continue;
        };
        let segments = name.segments();
        let Some(bare) = segments.last() else {
            continue;
        };
        if bare.is_empty() {
            continue;
        }
        ctx.add_known_symbol(bare);
        // The `AMI4CCM_` variant explicitly, so the resolver handles a
        // user-declared `AMI4CCM_<X>` cleanly.
        ctx.add_known_symbol(&format!("AMI4CCM_{bare}"));
        if segments.len() > 1 {
            let scope = segments[..segments.len() - 1].join("::");
            ctx.add_known_symbol(&format!("{scope}::{bare}"));
            ctx.add_known_symbol(&format!("{scope}::AMI4CCM_{bare}"));
        }
        let _ = bare.to_string();
    }
}

/// A fresh context filled from a model.
#[must_use]
pub fn context_from_model(ast: &Ast) -> TransformContext {
    let mut ctx = TransformContext::default();
    populate_from_model(&mut ctx, ast);
    ctx
}
