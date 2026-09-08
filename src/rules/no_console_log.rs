use crate::{
    ast::callee_name,
    rule::{Rule, RuleContext},
};
use oxc_ast::AstKind;
use oxc_semantic::Semantic;

pub struct NoConsoleLog;
impl Rule for NoConsoleLog {
    fn id(&self) -> &'static str {
        "no-console-log"
    }
    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for node in semantic.nodes().iter() {
            if let AstKind::CallExpression(call) = node.kind()
                && callee_name(&call.callee).as_deref() == Some("console.log")
            {
                context.report(call.span, "디버깅용 console.log 호출을 제거하세요");
            }
        }
    }
}
