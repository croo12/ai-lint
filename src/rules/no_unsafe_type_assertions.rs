use crate::rule::{Rule, RuleContext};
use oxc_ast::AstKind;
use oxc_semantic::Semantic;
use oxc_span::GetSpan;

pub struct NoUnsafeTypeAssertions;

impl Rule for NoUnsafeTypeAssertions {
    fn id(&self) -> &'static str {
        "no-unsafe-type-assertions"
    }

    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        let source = semantic.source_text();
        for node in semantic.nodes().iter() {
            let AstKind::TSAsExpression(assertion) = node.kind() else {
                continue;
            };
            let text = &source[assertion.span.start as usize..assertion.span.end as usize];
            let asserted_type = &source[assertion.type_annotation.span().start as usize
                ..assertion.type_annotation.span().end as usize];
            if asserted_type.trim() == "const" {
                continue;
            }
            if text.to_ascii_lowercase().contains("as unknown as") {
                context.report(
                    assertion.span,
                    "as unknown as는 금지합니다. 타입 경계를 명시적으로 검증하거나 변환하세요.",
                );
                continue;
            }
            let line_start = source[..assertion.span.start as usize]
                .rfind('\n')
                .map_or(0, |i| i + 1);
            let before = source[..line_start].trim_end();
            let previous_line = before.rsplit('\n').next().unwrap_or("").trim();
            if !previous_line.starts_with("//") && !previous_line.ends_with("*/") {
                context.report(
                    assertion.span,
                    "as 타입 단언에는 바로 위에 단언 이유를 설명하는 주석이 필요합니다.",
                );
            }
        }
    }
}
