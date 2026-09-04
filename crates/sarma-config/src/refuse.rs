//! The refusals that have to happen before evaluation, because they cannot
//! happen during it.
//!
//! Starlark defines equality across types as `False` rather than as an error,
//! and decides it without asking either value. That is correct for the language
//! and a hole here: `if db.ip == "10.0.0.1":` is silently false, the branch
//! silently does not happen, and a resource silently does not exist. No value
//! can intervene, because no value is consulted.
//!
//! Containment has the same shape. Ordering, indexing and iteration do not —
//! those reach the value and are refused there, with a line, in [`crate::handle`].
//!
//! So this walks the syntax tree first and refuses the two operations that would
//! otherwise pass unnoticed. It is deliberately coarse: **any** bare attribute
//! access compared for equality or containment is rejected, whether or not it
//! turns out to be a resource. In this language a bare `x.y` is a resource
//! attribute; a method call is `x.y()`, an item is `x["y"]`, and neither is
//! touched. A false positive costs somebody a rewrite of one line, and the
//! alternative costs them a resource that quietly failed to exist.

use starlark::syntax::AstModule;
use starlark_syntax::syntax::ast::{AstExprP, AstNoPayload, BinOp, ExprP};
use starlark_syntax::syntax::uniplate::Visit;

/// Refuses the comparisons that evaluation cannot catch.
///
/// Returns one message per offending comparison, in source order.
// ENFORCES: INV-PLAN-001
pub(crate) fn comparisons(module: &AstModule) -> Vec<String> {
    let mut refusals = Vec::new();
    walk(module, &Visit::Stmt(module.statement()), &mut refusals);
    refusals
}

fn walk(module: &AstModule, node: &Visit<'_, AstNoPayload>, refusals: &mut Vec<String>) {
    if let Visit::Expr(expression) = node
        && let ExprP::Op(left, operator, right) = &expression.node
        && let Some(named) = unsafe_comparison(*operator, left, right)
    {
        refusals.push(format!(
            "{}: {named} will not exist until apply, and comparing it with {} is \
             silently false rather than an error. Pass the value to another \
             resource instead of testing it",
            module.file_span(expression.span),
            describe(*operator)
        ));
    }

    node.visit_children(|child| walk(module, &child, refusals));
}

/// The attribute being compared, if this is a comparison that cannot be caught.
fn unsafe_comparison(
    operator: BinOp,
    left: &AstExprP<AstNoPayload>,
    right: &AstExprP<AstNoPayload>,
) -> Option<String> {
    if !matches!(
        operator,
        BinOp::Equal | BinOp::NotEqual | BinOp::In | BinOp::NotIn
    ) {
        return None;
    }
    attribute_path(left).or_else(|| attribute_path(right))
}

/// `db.ip` reads back as `db.ip`; anything else is not a bare attribute access.
fn attribute_path(expression: &AstExprP<AstNoPayload>) -> Option<String> {
    let ExprP::Dot(receiver, attribute) = &expression.node else {
        return None;
    };
    let base = match &receiver.node {
        ExprP::Identifier(name) => name.node.ident.clone(),
        ExprP::Dot(..) => attribute_path(receiver)?,
        _ => return None,
    };
    Some(format!("{base}.{}", attribute.node))
}

fn describe(operator: BinOp) -> &'static str {
    match operator {
        BinOp::Equal => "==",
        BinOp::NotEqual => "!=",
        BinOp::In => "in",
        _ => "not in",
    }
}
