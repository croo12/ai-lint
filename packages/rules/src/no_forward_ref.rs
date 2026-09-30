//! React 19 passes `ref` as an ordinary prop, so `forwardRef` is no longer needed.
use super::contract::{Rule, RuleContext};
use super::utils::ast::calls_module_export;
use oxc_ast::AstKind;
use oxc_semantic::Semantic;

const MESSAGE: &str = "forwardRef는 deprecated되었습니다. ref를 일반 prop으로 받으세요.";

pub struct NoForwardRef;

impl Rule for NoForwardRef {
    fn id(&self) -> &'static str {
        "no-forward-ref"
    }

    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for node in semantic.nodes().iter() {
            if let AstKind::CallExpression(call) = node.kind()
                && calls_module_export(semantic, &call.callee, "react", "forwardRef")
            {
                context.report(call.span, MESSAGE);
            }
        }
    }
}
