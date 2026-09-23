//! Route every React context through the project's createSafeContext factory.
use crate::{
    ast::calls_module_export,
    rule::{Rule, RuleContext},
};
use oxc_ast::AstKind;
use oxc_semantic::{AstNode, Semantic};

const MESSAGE: &str = "createContext를 직접 사용하지 마세요. createSafeContext를 사용하세요.";

pub struct NoCreateContext;

impl Rule for NoCreateContext {
    fn id(&self) -> &'static str {
        "no-create-context"
    }

    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for node in semantic.nodes().iter() {
            if let AstKind::CallExpression(call) = node.kind()
                && calls_module_export(semantic, &call.callee, "react", "createContext")
                && !inside_safe_context_factory(semantic, node)
            {
                context.report(call.span, MESSAGE);
            }
        }
    }
}

/// createSafeContext has to call createContext itself, so its own body stays allowed.
fn inside_safe_context_factory(semantic: &Semantic<'_>, node: &AstNode<'_>) -> bool {
    semantic
        .nodes()
        .ancestor_kinds(node.id())
        .any(|kind| match kind {
            AstKind::Function(function) => function
                .id
                .as_ref()
                .is_some_and(|id| id.name == "createSafeContext"),
            AstKind::VariableDeclarator(declarator) => declarator
                .id
                .get_binding_identifier()
                .is_some_and(|id| id.name == "createSafeContext"),
            _ => false,
        })
}
