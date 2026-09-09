use crate::rule::{Rule, RuleContext};
use oxc_ast::AstKind;
use oxc_semantic::Semantic;

/// Require an explicit public API instead of forwarding an entire module.
pub struct NoWildcardExport;

impl Rule for NoWildcardExport {
    fn id(&self) -> &'static str {
        "no-wildcard-export"
    }

    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        for node in semantic.nodes().iter() {
            if let AstKind::ExportAllDeclaration(export) = node.kind() {
                context.report(
                    export.span,
                    "와일드카드 export를 사용하지 마세요. 외부에서 필요한 항목만 이름을 명시하여 export하세요.",
                );
            }
        }
    }
}
