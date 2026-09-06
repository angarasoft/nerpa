//! The refusals that have to happen before evaluation, because they cannot
//! happen during it.
//!
//! Starlark defines equality across types as `False` rather than as an error,
//! and — worse — compiles a comparison against a constant into an instruction
//! that compares bytes and never consults either value at all. So
//! `if db.ip == "10.0.0.1":` is silently false, the branch silently does not
//! happen, and a resource silently does not exist. No value can intervene,
//! because no value is asked.
//!
//! Containment has the same shape. Ordering, indexing and iteration do not —
//! those reach the value and are refused there, with a line, in [`crate::handle`].
//!
//! # Why this is an analysis rather than a pattern
//!
//! It used to match one shape: a bare `x.y` beside `==`. That is the shape
//! people write, and it is not the only one reachable. Put the value in a
//! dictionary and read it back, hand it out of a function, iterate a list it is
//! in, and the pattern sees nothing while the branch is taken just the same.
//!
//! So the taint is followed instead. A value is **opaque** if it will not exist
//! until apply or if it is a secret; opacity spreads from the expression that
//! produces it through assignments, containers, indexing, iteration, method
//! calls, arguments, lambda bodies and function returns, and any comparison with
//! an opaque side is refused.
//!
//! Twice now the list has grown after somebody found what was not on it. The
//! first review found the dictionary read back with `[...]`; the second found
//! the same dictionary read back with `.get`, which the first fix had walked
//! straight past. The list is written out above rather than described, because
//! the failure both times was a shape nobody had written down.
//!
//! # What it deliberately over-refuses
//!
//! Coarse in the same direction as before, and for the same trade. **Any** bare
//! attribute access is treated as a resource attribute, whether or not it turns
//! out to be one — in this language a bare `x.y` is a resource attribute, while
//! a method call is `x.y()` and an item is `x["y"]`. A name that ever held an
//! opaque value is treated as holding one everywhere, including after it has
//! been rebound to something ordinary. A container with one opaque element is
//! opaque whole. A false positive costs somebody a rewrite of one line; the
//! alternative costs them a resource that quietly failed to exist.
//!
//! # What it still does not catch
//!
//! This is a static over-approximation, not a proof. An opaque value that
//! reaches a comparison through a function *parameter* is not followed, because
//! that needs the call sites of every function and this does not have them —
//! including the case where the function is an identity that hands the value
//! back into the module. The guarantee is worth exactly what the analysis is
//! worth, and saying so is the point of saying it here.

use std::collections::BTreeMap;

use starlark::syntax::AstModule;
use starlark_syntax::syntax::ast::{
    ArgumentP, AssignTargetP, AstArgumentP, AstExprP, AstNoPayload, AstStmtP, BinOp, ClauseP,
    ExprP, StmtP,
};
use starlark_syntax::syntax::uniplate::Visit;

/// The global whose result must never be looked at.
const SECRET: &str = "secret";

/// Why a value must not be compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opaque {
    /// It will not exist until apply.
    Deferred,
    /// It is a secret, and the value is deliberately somewhere else.
    Secret,
}

/// Names — variables, loop variables, functions — known to carry an opaque value.
type Tainted = BTreeMap<String, Opaque>;

/// Refuses the comparisons that evaluation cannot catch.
///
/// Returns one message per offending comparison, in source order.
// ENFORCES: INV-PLAN-001
// ENFORCES: INV-SECRET-004
pub(crate) fn comparisons(module: &AstModule) -> Vec<String> {
    let mut tainted = Tainted::new();
    // A name can be bound after it is read and a function can call one defined
    // below it, so one pass is not enough. Taint only ever grows and names are
    // finite, so this settles.
    loop {
        let before = tainted.len();
        spread(&Visit::Stmt(module.statement()), &mut tainted);
        if tainted.len() == before {
            break;
        }
    }

    let mut refusals = Vec::new();
    walk(
        module,
        &tainted,
        &Visit::Stmt(module.statement()),
        &mut refusals,
    );
    refusals
}

/// Follows opacity from the expressions that produce it into the names that hold it.
fn spread(node: &Visit<'_, AstNoPayload>, tainted: &mut Tainted) {
    if let Visit::Stmt(statement) = node {
        match &statement.node {
            StmtP::Assign(assignment) => {
                bind(
                    &assignment.lhs.node,
                    taint(&assignment.rhs, tainted),
                    tainted,
                );
            }
            StmtP::AssignModify(target, _, rhs) => {
                bind(&target.node, taint(rhs, tainted), tainted);
            }
            // Iterating an opaque container hands out opaque elements.
            StmtP::For(loop_) => {
                bind(&loop_.var.node, taint(&loop_.over, tainted), tainted);
            }
            // A function that can hand one back is opaque wherever it is called.
            StmtP::Def(definition) => {
                if let Some(kind) = returned(&definition.body, tainted) {
                    tainted.entry(definition.name.ident.clone()).or_insert(kind);
                }
            }
            _ => {}
        }
    }

    // A comprehension binds its variable inside an expression rather than with a
    // `for` statement, so the arm above never sees it. Missed once, and found by
    // an adversarial test rather than by reasoning.
    if let Visit::Expr(expression) = node {
        match &expression.node {
            ExprP::ListComprehension(_, clause, rest)
            | ExprP::DictComprehension(_, clause, rest) => {
                bind(&clause.var.node, taint(&clause.over, tainted), tainted);
                for further in rest {
                    if let ClauseP::For(clause) = further {
                        bind(&clause.var.node, taint(&clause.over, tainted), tainted);
                    }
                }
            }
            _ => {}
        }
    }

    node.visit_children(|child| spread(&child, tainted));
}

/// What a function body can return, if any of it is opaque.
fn returned(body: &AstStmtP<AstNoPayload>, tainted: &Tainted) -> Option<Opaque> {
    let mut found = None;
    returns(&Visit::Stmt(body), tainted, &mut found);
    found
}

fn returns(node: &Visit<'_, AstNoPayload>, tainted: &Tainted, found: &mut Option<Opaque>) {
    if found.is_none()
        && let Visit::Stmt(statement) = node
        && let StmtP::Return(Some(expression)) = &statement.node
    {
        *found = taint(expression, tainted);
    }

    node.visit_children(|child| returns(&child, tainted, found));
}

/// Records a name as carrying an opaque value.
fn bind(target: &AssignTargetP<AstNoPayload>, kind: Option<Opaque>, tainted: &mut Tainted) {
    let Some(kind) = kind else {
        return;
    };
    match target {
        AssignTargetP::Identifier(name) => {
            tainted.entry(name.node.ident.clone()).or_insert(kind);
        }
        // `a, b = pair`: nothing here knows which half carried it, so both do.
        AssignTargetP::Tuple(parts) => {
            for part in parts {
                bind(&part.node, Some(kind), tainted);
            }
        }
        AssignTargetP::Index(..) | AssignTargetP::Dot(..) => {}
    }
}

/// Whether an expression can produce a value that must not be compared.
fn taint(expression: &AstExprP<AstNoPayload>, tainted: &Tainted) -> Option<Opaque> {
    match &expression.node {
        // In this language a bare `x.y` is a resource attribute.
        ExprP::Dot(..) => Some(Opaque::Deferred),
        ExprP::Identifier(name) => tainted.get(&name.node.ident).copied(),
        ExprP::Call(callee, arguments) => {
            let held = match &callee.node {
                ExprP::Identifier(name) if name.node.ident == SECRET => Some(Opaque::Secret),
                // A method call reads nothing from a resource *itself* — but it
                // hands back what the receiver was holding, and a dictionary
                // gives its value up to `.get` exactly as readily as to `[...]`.
                // Taking the receiver rather than the whole `x.y` matters: a
                // bare `x.y` is a resource attribute and always opaque, so
                // asking about it here would make every method call on every
                // string opaque too.
                ExprP::Dot(receiver, _) => taint(receiver, tainted),
                _ => taint(callee, tainted),
            };
            // And what it was handed, for the method that gives an argument
            // back rather than something it held: `held.get(key, fallback)`.
            held.or_else(|| {
                arguments
                    .args
                    .iter()
                    .find_map(|argument| handed(argument, tainted))
            })
        }
        ExprP::Index(pair) => taint(&pair.0, tainted),
        ExprP::Index2(triple) => taint(&triple.0, tainted),
        ExprP::Slice(base, ..) => taint(base, tainted),
        ExprP::List(items) | ExprP::Tuple(items) => {
            items.iter().find_map(|item| taint(item, tainted))
        }
        ExprP::Dict(pairs) => pairs
            .iter()
            .find_map(|(key, value)| taint(key, tainted).or_else(|| taint(value, tainted))),
        ExprP::Op(left, _, right) => taint(left, tainted).or_else(|| taint(right, tainted)),
        ExprP::If(triple) => taint(&triple.1, tainted).or_else(|| taint(&triple.2, tainted)),
        ExprP::Not(inner) | ExprP::Minus(inner) | ExprP::Plus(inner) | ExprP::BitNot(inner) => {
            taint(inner, tainted)
        }
        ExprP::ListComprehension(element, clause, _) => {
            taint(element, tainted).or_else(|| taint(&clause.over, tainted))
        }
        ExprP::DictComprehension(pair, clause, _) => taint(&pair.0, tainted)
            .or_else(|| taint(&pair.1, tainted))
            .or_else(|| taint(&clause.over, tainted)),
        // A `def` body is walked for what it returns; a lambda is a body with
        // nothing else in it, and not walking it made the shortest way to write
        // the same thing the one way through.
        ExprP::Lambda(lambda) => taint(&lambda.body, tainted),
        ExprP::Literal(_) | ExprP::FString(_) => None,
    }
}

/// What an argument carries into a call.
fn handed(argument: &AstArgumentP<AstNoPayload>, tainted: &Tainted) -> Option<Opaque> {
    match &argument.node {
        ArgumentP::Positional(expression)
        | ArgumentP::Named(_, expression)
        | ArgumentP::Args(expression)
        | ArgumentP::KwArgs(expression) => taint(expression, tainted),
    }
}

fn walk(
    module: &AstModule,
    tainted: &Tainted,
    node: &Visit<'_, AstNoPayload>,
    refusals: &mut Vec<String>,
) {
    if let Visit::Expr(expression) = node
        && let ExprP::Op(left, operator, right) = &expression.node
        && matches!(
            operator,
            BinOp::Equal | BinOp::NotEqual | BinOp::In | BinOp::NotIn
        )
        && let Some((kind, named)) = offending(left, right, tainted)
    {
        let complaint = match kind {
            Opaque::Deferred => format!(
                "{named} will not exist until apply, and comparing it with {} is \
                 silently false rather than an error. Pass the value to another \
                 resource instead of testing it",
                describe(*operator)
            ),
            Opaque::Secret => format!(
                "{named} is a secret, and comparing it with {} is silently false \
                 rather than an error. Pass it to a resource instead of testing it",
                describe(*operator)
            ),
        };
        refusals.push(format!(
            "{}: {complaint}",
            module.file_span(expression.span)
        ));
    }

    node.visit_children(|child| walk(module, tainted, &child, refusals));
}

/// The opaque side of a comparison, and how to name it in the message.
fn offending(
    left: &AstExprP<AstNoPayload>,
    right: &AstExprP<AstNoPayload>,
    tainted: &Tainted,
) -> Option<(Opaque, String)> {
    [left, right]
        .into_iter()
        .find_map(|side| taint(side, tainted).map(|kind| (kind, name_of(side))))
}

/// How an expression should be referred to in a refusal.
fn name_of(expression: &AstExprP<AstNoPayload>) -> String {
    match &expression.node {
        ExprP::Identifier(name) => name.node.ident.clone(),
        ExprP::Dot(..) => attribute_path(expression).unwrap_or_else(|| "this value".to_owned()),
        ExprP::Call(callee, _) => match &callee.node {
            ExprP::Identifier(name) => format!("{}(...)", name.node.ident),
            _ => "this value".to_owned(),
        },
        ExprP::Index(pair) => format!("{}[...]", name_of(&pair.0)),
        ExprP::Index2(triple) => format!("{}[...]", name_of(&triple.0)),
        ExprP::Slice(base, ..) => format!("{}[...]", name_of(base)),
        _ => "this value".to_owned(),
    }
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
        BinOp::NotIn => "not in",
        _ => "that comparison",
    }
}
