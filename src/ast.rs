//! Shared typed AST helpers; rule policies live in `rules`.
use oxc_ast::{AstKind, ast::Expression};
use oxc_semantic::{AstNode, Semantic};
use oxc_span::{GetSpan, Span};

pub fn callee_name(expression: &Expression<'_>) -> Option<String> {
    match expression.get_inner_expression() {
        Expression::Identifier(id) => Some(id.name.to_string()),
        Expression::StaticMemberExpression(member) => Some(format!(
            "{}.{}",
            callee_name(&member.object)?,
            member.property.name
        )),
        _ => None,
    }
}

/// True when the callee is `name`, `<object>.name`, or a binding renamed from an import of `name`.
pub fn calls_imported(semantic: &Semantic<'_>, callee: &Expression<'_>, name: &str) -> bool {
    callee_name(callee).is_some_and(|called| {
        called == name
            || called
                .strip_suffix(name)
                .is_some_and(|object| object.ends_with('.'))
    }) || imported_name(semantic, callee).is_some_and(|imported| imported == name)
}

/// The name a binding was imported under, before any local rename.
pub fn imported_name(semantic: &Semantic<'_>, expression: &Expression<'_>) -> Option<String> {
    let Expression::Identifier(id) = expression.get_inner_expression() else {
        return None;
    };
    let symbol = semantic
        .scoping()
        .get_reference(id.reference_id.get()?)
        .symbol_id()?;
    match semantic
        .nodes()
        .kind(semantic.scoping().symbol_declaration(symbol))
    {
        AstKind::ImportSpecifier(import) => Some(import.imported.name().to_string()),
        _ => None,
    }
}

pub fn reference_symbol(semantic: &Semantic<'_>, expression: &Expression<'_>) -> Option<usize> {
    let Expression::Identifier(id) = expression.get_inner_expression() else {
        return None;
    };
    id.reference_id
        .get()
        .and_then(|id| semantic.scoping().get_reference(id).symbol_id())
        .map(|id| id.index())
}

pub fn is_function(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
    )
}

pub fn enclosing_function(semantic: &Semantic<'_>, node: &AstNode<'_>) -> Option<Span> {
    semantic
        .nodes()
        .ancestor_kinds(node.id())
        .find(|kind| is_function(*kind))
        .map(|kind| kind.span())
}

/// Class initializers are outside an enclosing function's own execution body.
pub fn local_function(semantic: &Semantic<'_>, node: &AstNode<'_>) -> Option<Span> {
    semantic
        .nodes()
        .ancestor_kinds(node.id())
        .find(|kind| is_function(*kind) || matches!(kind, AstKind::Class(_)))
        .filter(|kind| is_function(*kind))
        .map(|kind| kind.span())
}

pub fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
pub fn excerpt(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

pub fn loop_body(kind: AstKind<'_>) -> Option<Span> {
    match kind {
        AstKind::ForStatement(node) => Some(node.body.span()),
        AstKind::ForOfStatement(node) => Some(node.body.span()),
        AstKind::ForInStatement(node) => Some(node.body.span()),
        AstKind::WhileStatement(node) => Some(node.body.span()),
        AstKind::DoWhileStatement(node) => Some(node.body.span()),
        _ => None,
    }
}
