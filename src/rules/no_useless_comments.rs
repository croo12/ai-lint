use crate::rule::{Rule, RuleContext};
use oxc_semantic::Semantic;

/// Reject comments that belong in code, issue trackers, or ADRs.
pub struct NoUselessComments;

impl Rule for NoUselessComments {
    fn id(&self) -> &'static str {
        "no-useless-comments"
    }

    fn check(&self, semantic: &Semantic<'_>, context: &mut RuleContext<'_>) {
        let source = semantic.source_text();
        for comment in semantic.comments() {
            let span = comment.content_span();
            let text = source[span.start as usize..span.end as usize]
                .lines()
                .map(|line| line.trim().trim_start_matches('*').trim())
                .collect::<Vec<_>>()
                .join(" ");
            let text = text.trim();
            let lower = text.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("todo")
                && (rest.is_empty() || rest.starts_with(|c: char| !c.is_alphanumeric() && c != '_'))
            {
                if rest.trim_matches([':', '-', ' ', '\t', '\n']).is_empty() {
                    context.report(comment.span, "TODO에는 구체적인 설명을 추가하세요.");
                }
                continue;
            }
            let kind = if (lower.contains("http://") || lower.contains("https://"))
                && !lower.contains("issue")
                && !lower.contains("pull")
                && !lower.contains("pr #")
            {
                Some("문서 링크는 이슈·ADR·문서로 이동하세요.")
            } else if ["because", "선택", "대신", "결정", "adr"]
                .iter()
                .any(|word| lower.contains(word))
            {
                Some("설계 결정과 선택 이유는 ADR에 기록하세요.")
            } else if lower.starts_with("const ")
                || lower.starts_with("return ")
                || lower.starts_with("if ")
                || lower.starts_with("for ")
                || lower.starts_with("this ")
                || lower.starts_with("call ")
            {
                Some("코드의 동작을 반복하는 주석은 제거하세요.")
            } else {
                None
            };
            if let Some(message) = kind {
                context.report(comment.span, message);
            }
        }
    }
}
