use crate::{
    ast::{callee_name, contains, enclosing_function, reference_symbol},
    rule::{Rule, RuleContext},
};
use oxc_ast::{
    AstKind,
    ast::{BindingPattern, Expression},
};
use oxc_semantic::Semantic;
use oxc_span::Span;
use std::collections::HashSet;

pub const MESSAGE: &str = "useEffect에 setState를 넣어서는 안됩니다";
pub struct NoSetStateInEffect;
impl Rule for NoSetStateInEffect {
    fn id(&self) -> &'static str {
        "no-set-state-in-effect"
    }
    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for (effect, _) in candidates(semantic) {
            context.report(effect, MESSAGE);
        }
    }
}

fn state_setters(semantic: &Semantic<'_>) -> HashSet<usize> {
    semantic
        .nodes()
        .iter()
        .filter_map(|node| {
            let AstKind::VariableDeclarator(declaration) = node.kind() else {
                return None;
            };
            let BindingPattern::ArrayPattern(pattern) = &declaration.id else {
                return None;
            };
            let Expression::CallExpression(init) =
                declaration.init.as_ref()?.get_inner_expression()
            else {
                return None;
            };
            if !matches!(
                callee_name(&init.callee).as_deref(),
                Some("useState" | "React.useState")
            ) {
                return None;
            }
            let Some(BindingPattern::BindingIdentifier(setter)) = pattern.elements.get(1)?.as_ref()
            else {
                return None;
            };
            setter.symbol_id.get().map(|id| id.index())
        })
        .collect()
}

/// One candidate per effect; nested callback functions retain the existing policy.
pub(super) fn candidates(semantic: &Semantic<'_>) -> Vec<(Span, Span)> {
    let setters = state_setters(semantic);
    let updates: Vec<_> = semantic
        .nodes()
        .iter()
        .filter_map(|node| {
            let AstKind::CallExpression(call) = node.kind() else {
                return None;
            };
            let is_setter = callee_name(&call.callee).as_deref() == Some("setState")
                || reference_symbol(semantic, &call.callee)
                    .is_some_and(|symbol| setters.contains(&symbol));
            is_setter.then_some(call.span)
        })
        .collect();
    semantic
        .nodes()
        .iter()
        .filter_map(|node| {
            let AstKind::CallExpression(call) = node.kind() else {
                return None;
            };
            if !matches!(
                callee_name(&call.callee).as_deref(),
                Some("useEffect" | "React.useEffect")
            ) {
                return None;
            }
            let callback = call
                .arguments
                .first()?
                .as_expression()?
                .get_inner_expression();
            let span = match callback {
                Expression::ArrowFunctionExpression(function) => function.span,
                Expression::FunctionExpression(function) => function.span,
                _ => return None,
            };
            updates
                .iter()
                .any(|update| contains(span, *update))
                .then(|| {
                    (
                        call.span,
                        enclosing_function(semantic, node).unwrap_or(call.span),
                    )
                })
        })
        .collect()
}
