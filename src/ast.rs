//! Shared typed AST helpers; rule policies live in `rules`.
use oxc_ast::{
    AstKind,
    ast::{BindingPattern, Expression, VariableDeclarator},
};
use oxc_semantic::{AstNode, NodeId, Semantic};
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

/// How a local binding entered the file from another module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Import {
    /// `import { name as local } from "module"`.
    Named { module: String, name: String },
    /// `import local from "module"` or `import * as local from "module"`.
    Whole { module: String },
}

/// Local aliases are followed one binding at a time; the cap stops a reference cycle.
const ALIAS_DEPTH: u8 = 12;

/// Resolves an identifier to the module export it denotes, through local aliases.
pub fn resolve_import(semantic: &Semantic<'_>, expression: &Expression<'_>) -> Option<Import> {
    resolve_alias(semantic, expression, 0)
}

fn resolve_alias(
    semantic: &Semantic<'_>,
    expression: &Expression<'_>,
    depth: u8,
) -> Option<Import> {
    if depth > ALIAS_DEPTH {
        return None;
    }
    let Expression::Identifier(id) = expression.get_inner_expression() else {
        return None;
    };
    let declaration = declaration_node(semantic, expression)?;
    let module = || {
        semantic
            .nodes()
            .ancestor_kinds(declaration)
            .find_map(|kind| match kind {
                AstKind::ImportDeclaration(import) => Some(import.source.value.to_string()),
                _ => None,
            })
    };
    match semantic.nodes().kind(declaration) {
        AstKind::ImportSpecifier(import) => Some(Import::Named {
            module: module()?,
            name: import.imported.name().to_string(),
        }),
        AstKind::ImportDefaultSpecifier(_) | AstKind::ImportNamespaceSpecifier(_) => {
            Some(Import::Whole { module: module()? })
        }
        AstKind::VariableDeclarator(declarator) => {
            resolve_binding(semantic, declarator, &id.name, depth)
        }
        _ => None,
    }
}

/// `const local = imported` and `const { name } = namespace` keep the original export.
fn resolve_binding(
    semantic: &Semantic<'_>,
    declarator: &VariableDeclarator<'_>,
    local: &str,
    depth: u8,
) -> Option<Import> {
    let init = declarator.init.as_ref()?;
    match &declarator.id {
        BindingPattern::BindingIdentifier(_) => resolve_alias(semantic, init, depth + 1),
        BindingPattern::ObjectPattern(pattern) => {
            let name = pattern.properties.iter().find_map(|property| {
                let binding = property.value.get_binding_identifier()?;
                if binding.name != local {
                    return None;
                }
                Some(property.key.static_name()?.into_owned())
            })?;
            match resolve_alias(semantic, init, depth + 1)? {
                Import::Whole { module } => Some(Import::Named { module, name }),
                Import::Named { .. } => None,
            }
        }
        _ => None,
    }
}

fn declaration_node(semantic: &Semantic<'_>, expression: &Expression<'_>) -> Option<NodeId> {
    let Expression::Identifier(id) = expression.get_inner_expression() else {
        return None;
    };
    let symbol = semantic
        .scoping()
        .get_reference(id.reference_id.get()?)
        .symbol_id()?;
    Some(semantic.scoping().symbol_declaration(symbol))
}

/// True when the callee is `name` as exported by `module`: a named import under any local
/// name, `<binding>.name` on a default or namespace import of `module`, or a local alias of
/// either. An identifier that resolves to no declaration at all falls back to a name match,
/// so a pasted fragment without its imports still reports.
pub fn calls_module_export(
    semantic: &Semantic<'_>,
    callee: &Expression<'_>,
    module: &str,
    name: &str,
) -> bool {
    match callee.get_inner_expression() {
        Expression::StaticMemberExpression(member) => {
            member.property.name == name
                && matches!(
                    resolve_import(semantic, &member.object),
                    Some(Import::Whole { module: source }) if source == module
                )
        }
        Expression::Identifier(id) => match resolve_import(semantic, callee) {
            Some(Import::Named {
                module: source,
                name: exported,
            }) => source == module && exported == name,
            Some(Import::Whole { .. }) => false,
            None => id.name == name && declaration_node(semantic, callee).is_none(),
        },
        _ => false,
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
