use crate::{
    ast::callee_name,
    rule::{Rule, RuleContext},
};
use oxc_ast::AstKind;
use oxc_semantic::Semantic;

pub struct NoAlert;
impl Rule for NoAlert {
    fn id(&self) -> &'static str {
        "no-alert"
    }
    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for node in semantic.nodes().iter() {
            if let AstKind::CallExpression(call) = node.kind()
                && matches!(
                    callee_name(&call.callee).as_deref(),
                    Some("alert" | "window.alert")
                )
            {
                context.report(
                    call.span,
                    "alert 대신 사용자 인터페이스에서 메시지를 표시하세요",
                );
            }
        }
    }
}
